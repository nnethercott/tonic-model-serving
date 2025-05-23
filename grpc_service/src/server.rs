use std::pin::Pin;

use crate::{
    Error, InferenceRequest, InferenceResponse, ModelSpec,
    config::Settings,
    inferencer_server::{Inferencer, InferencerServer},
};
use inference_core::modelpool::{ModelPool, SendBackMessage};
use sqlx::{PgPool, QueryBuilder};
use tokio::sync::{Mutex, mpsc};
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, transport::Server};
use tower_http::trace::{MakeSpan, TraceLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

// TODO: add a job that runs when new models are added to download

pub struct ModelServer {
    pub registry: Mutex<Vec<ModelSpec>>,
    pub model_pool: ModelPool,
    pg_pool: PgPool,
}

impl ModelServer {
    pub fn new(pg_pool: PgPool, model_pool: ModelPool) -> anyhow::Result<Self> {
        Ok(ModelServer {
            pg_pool,
            model_pool,
            registry: Mutex::new(vec![]),
        })
    }

    async fn fetch_models(&self) -> sqlx::Result<()> {
        let models: Vec<ModelSpec> = sqlx::query_as::<_, ModelSpec>(r#"SELECT * FROM MODELS"#)
            .fetch_all(&self.pg_pool)
            .await?;

        *(self.registry.lock().await) = models;

        Ok(())
    }

    async fn add_models(&self, models: Vec<ModelSpec>) -> sqlx::Result<u64> {
        let mut query_builder = QueryBuilder::new("INSERT INTO models(model_id, model_type)");

        // todo! maybe look into unnest
        query_builder.push_values(models, |mut b, model| {
            b.push_bind(model.model_id).push_bind(model.model_type);
        });

        let n_rows = query_builder
            .build()
            .execute(&self.pg_pool)
            .await?
            .rows_affected();

        dbg!(n_rows);

        // refresh registry with new models
        self.fetch_models().await?;
        Ok(n_rows)
    }
}

#[tonic::async_trait]
impl Inferencer for ModelServer {
    async fn run_inference(
        &self,
        _request: Request<InferenceRequest>,
    ) -> Result<Response<InferenceResponse>, Status> {
        // use onnx inference from crate we haven't defined yet ...
        todo!()
    }

    #[doc = "Server streaming response type for the ListModels method."]
    type ListModelsStream = ReceiverStream<Result<ModelSpec, Status>>;

    async fn list_models(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::ListModelsStream>, Status> {
        let (tx, rx) = mpsc::channel(4);

        let model_list = { self.registry.lock().await.clone() };

        tokio::spawn(async move {
            for spec in model_list {
                tx.send(Ok(spec)).await.unwrap();
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[doc = "rpc runBatchedInference(stream InferenceRequest) returns (stream InferenceResponse);"]
    async fn add_models(
        &self,
        request: Request<tonic::Streaming<ModelSpec>>,
    ) -> Result<Response<u64>, Status> {
        let models: Vec<ModelSpec> = request.into_inner().filter_map(|i| i.ok()).collect().await;

        let n_rows = self.add_models(models).await.unwrap_or_else(|e| {
            error!(error=?e);
            warn!("0 models added");
            0
        });

        Ok(Response::new(n_rows))
    }

    #[doc = " Server streaming response type for the GenerateStreaming method."]
    type GenerateStreamingStream =
        Pin<Box<dyn Stream<Item = Result<String, Status>> + Send + Sync + 'static>>;

    async fn generate_streaming(
        &self,
        request: Request<String>,
    ) -> std::result::Result<Response<Self::GenerateStreamingStream>, Status> {
        let prompt = request.into_inner();

        let (tx, rx) = mpsc::channel(1024);
        let req = SendBackMessage::Streaming { prompt, sender: tx };

        // schedule inference job
        self.model_pool.infer(req).unwrap();

        // Result<u32, Status> is a constraint from tonic; we need to adapt the rx token stream
        // into this expected format
        let adpt = ReceiverStream::new(rx).map(Ok);

        Ok(Response::new(Box::pin(adpt)))
    }
}

#[derive(Clone)]
struct ServerMakeSpan;

/// span for logging incoming requests to the server
impl<T> MakeSpan<T> for ServerMakeSpan {
    fn make_span(&mut self, request: &http::Request<T>) -> tracing::Span {
        tracing::span!(
            tracing::Level::INFO,
            "tonic_grpc_request",
            method= %request.method(),
            uri = %request.uri().path(),
            span_id = %Uuid::new_v4(), // FIXME: hash this and hexdump
        )
    }
}

pub async fn run_server(config: Settings, model_pool: ModelPool) -> Result<(), Error> {
    let socket_addr = config.server.addr().parse().unwrap();

    // health
    let (reporter, health_service) = tonic_health::server::health_reporter();
    reporter
        .set_serving::<InferencerServer<ModelServer>>()
        .await;

    // reflection service
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(crate::FILE_DESCRIPTOR_SET)
        .build_v1alpha()
        .unwrap();

    // connect to db and refresh models
    let model_server = ModelServer::new(config.db.create_pool(), model_pool)?;
    if let Err(e) = model_server.fetch_models().await {
        error!(error=%e);
    }

    let server = Server::builder()
        // add tower tracing layer for requests
        .layer(TraceLayer::new_for_grpc().make_span_with(ServerMakeSpan))
        // add service layers -> [ml, reflection, health]
        .add_service(InferencerServer::new(model_server))
        .add_service(reflection_service)
        .add_service(health_service);

    info!("starting server...");
    // info!(config=?config);

    server
        .serve(socket_addr)
        .await
        .map_err(|e| Status::internal(format!("server failed to start: {e}")))?;

    Ok(())
}

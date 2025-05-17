# chat-rs
LLM model serving from scratch in pure Rust. Made with tonic, axum, candle.rs, and ❤️

Currently a work in progress ... 

# Notes to self 
## flow 
- local dev
  - sqlx to apply migrations to postgres image running in docker (so far a la zero2prod but its a nice pattern, sue me)

## goals & ideas
tangible:
- implement inference server based on gRPC ✅
- k8s deployment + observability to check balancing with load tests
- better telemetry (otel, tracing)
- tracing and structured logging ✅
- health check with probing (added later in the deployment to ensure service up and running) ✅
- **minio** in deployment as a bucket for storing model weights
- gRPC health probe k8s ✅
  - there's also [this](https://github.com/grpc-ecosystem/grpc-health-probe) cli tool
  - another health endpoint would be the axum server itself
- pipe/sync logs to an elasticsearch instance ?
- [rust-cache](https://github.com/Swatinem/rust-cache) for reducing gh action times ✅
- *graceful shutdown* for grpc and web services ✅
- project pivot idea: use the service as an embeddings type generation thing like meilisearch and implement a database
  - could try to do an from-scratch implementation of ann
  - **or** take inspo and use thread/process pool on worker machine to handle incoming inference requests
- lightweight models would be better for testing locally; *embedding*, xgboost, time series models, <1b llms, etc.
  - need a **use case**

## web 
- TODO: figure out how we can configure num replicas of the site ...
  - is this done at the tokio level or through k8s deployments
  - for each replica we'll need an internal IP used in the pod network, would this come from an env variable override of our config through a ConfigMap?
- can use a redis or sqlite instance tied to the session id  of a user to store conversation history server-side
  - might need a storage layer ...
- share the grpc client through an app state ✅
- grafana + otel + prometheus for metrics like latency, token throughput, etc
- in a v2: websocket or chunked in routes/chat to simulate real-time chat

### notes on logging
- tracing layer for grpc like [this](https://docs.rs/tower-http/latest/tower_http/trace/struct.TraceLayer.html#method.new_for_grpc)  
  - or we can use the `interceptor` in the server init to inject the logging middleware
- we probably want 1) gRPC request-level logs, and 2) logging the internals through #\[instrument\]

conceptual:
- familiarize myself with tokio ecosystem (tonic, hyper, axum)
- review async rust
- multi-service k8s deployments (tonic grpc server, typescript(?) frontend, axum backend, db, buckets)

## todos 
- [x] [grpc basics](https://grpc.io/docs/languages/python/basics/) with examples 
- [x] skim tonic docs
- [x] add test actions and branch protection rules 
- [x] cargo workspace setup
- [x] re-read docs on streams and futures in rust (see if we can avoid the ReceiverStream pattern)
- [x] setup health probe alongside service with tonic-health [docs](https://github.com/hyperium/tonic/tree/master/examples/src/health) 
- [x] setup db to store model registry (could this be replaced down the line with mlflow?)
- [x] add reflection
- [x] better tests
- [x] add env files serializing to app config
- [x] tracing and formatted logs
- [x] llvm linker
  - [x] read that article on minimizing build times
- [x] worker pool for models to serve requests (stateless with redis)
  - [ ] redis middleware not yet done
- [x] graceful shutdown
- [x] basic inference
- [x] streaming inference
- [ ] update proto; define interface for streaming/blocking requests
- [x] move model pool spawn outside of tokio context so we can blocking_send
- [ ] add clap to all the configs
  - [ ] serde + serde_yaml for deserializing from a local file ?
  - [ ] register relevant environment variables for ports and stuff


## notes 
- to run the grpc server and client run `cargo run --bin server` in one terminal, and `cargo run --bin client` in another
- running web is done with `cargo run --bin web`
- to run a health check first spin up the server with `cargo run --bin server` then `grpc-health-probe -addr="[::1]:50051"`
- to inspect the API we can use Postman with gRPC reflection

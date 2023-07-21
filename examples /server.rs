[package]
name = "netsrv"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
tokio = { version = "1.28", features = ["macros", "rt-multi-thread"] }
tokio-stream = "0.1.14"
tonic = { version = "0.9.2" }
prost = "0.11.9"
tracing = { version = "0.1.37", optional = true }
tracing-subscriber = { version = "0.3.17", features = ["tracing-log", "fmt"], optional = true }
bytes = { version = "1.4.0", optional = true }
http = { version = "0.2.9", optional = true }
http-body = { version = "1.0.0-rc1", optional = true }
hyper = { version = "0.14.26", optional = true }
h2 = { version = "0.3.19", optional = true }
serde = { version = "1.0.163", deatures = ["derive"] }
serde_json = { version = "1.0.96" }
prost-types = { version = "0.11.9", optional = true }
async-stream = "0.3.5"
rand = "0.8.5"
rand_distr = "0.4.3"
axum = "0.6.18"
regex = "1.9.1"
reqwest = { version = "0.11.18", features = ["h3", "json"] }

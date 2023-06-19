use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::{self, wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status, Streaming};

use greet::{
    greeter_server::{Greeter, GreeterServer},
    HelloReq, HelloResp,
};
use voting::{
    voting_server::{Voting, VotingServer},
    VotingRequest, VotingResponse,
};
use web::{
    web_server::{Web, WebServer},
    WebRequest, WebResponse,
};

pub mod voting {
    include!("../protos/voting.rs");
}

pub mod greet {
    include!("../protos/hello.rs");
}

pub mod web {
    include!("../protos/web.rs");
}

#[derive(Debug, Default)]
pub struct VotingService;

#[tonic::async_trait]
impl Voting for VotingService {
    async fn vote(
        &self,
        request: Request<VotingRequest>,
    ) -> Result<Response<VotingResponse>, Status> {
        let req: &VotingRequest = request.get_ref();
        match req.vote {
            0 => Ok(Response::new(VotingResponse {
                confirmation: format!("upvoted  for {}", req.url),
            })),
            1 => Ok(Response::new(VotingResponse {
                confirmation: format!("downvoted for {}", req.url),
            })),
            _ => Err(Status::new(
                tonic::Code::OutOfRange,
                "Invalid vote provided",
            )),
        }
    }
}

#[derive(Debug, Default)]
pub struct GreetService;

#[tonic::async_trait]
impl Greeter for GreetService {
    async fn say_hello(&self, request: Request<HelloReq>) -> Result<Response<HelloResp>, Status> {
        let hello_str = request.into_inner().content;
        println!("greet from client: {}", hello_str);

        Ok(Response::new(HelloResp { content: hello_str }))
    }
}

#[derive(Debug, Default)]
pub struct WebService {
    features: Arc<Vec<WebResponse>>,
}

#[tonic::async_trait]
impl Web for WebService {
    async fn unary_web(
        &self,
        request: Request<WebRequest>,
    ) -> Result<Response<WebResponse>, Status> {
        let req = request.into_inner();
        if &req.id == &0i64 {
            return Err(Status::invalid_argument("参数错误"));
        }

        Ok(Response::new(WebResponse {
            code: 200,
            message: "请求成功".to_string(),
        }))
    }

    type ServerSteamingWebStream = ReceiverStream<Result<WebResponse, Status>>;

    async fn server_steaming_web(
        &self,
        _request: Request<WebRequest>,
    ) -> Result<Response<Self::ServerSteamingWebStream>, Status> {
        let (tx, rx): (
            Sender<Result<WebResponse, Status>>,
            Receiver<Result<WebResponse, Status>>,
        ) = tokio::sync::mpsc::channel(15);
        let features = self.features.clone();

        tokio::spawn(async move {
            for web in &features[..] {
                tx.send(Ok(web.clone())).await.unwrap();
            }

            println!(" /// done sending...");
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn client_streaming_web(
        &self,
        request: Request<Streaming<WebRequest>>,
    ) -> Result<Response<WebResponse>, Status> {
        let mut response = WebResponse {
            code: 200,
            message: "".to_string(),
        };

        let mut req = request.into_inner();
        while let Some(req) = req.next().await {
            let result = req?;

            response.message = format!("{}, {}", response.message, result.id);
        }

        /*
        // 第二种写法
        request
        .into_inner()
        .fold(response, |mut acc, input| {
            let input = input.unwrap();

            acc.code += input.id;
            acc.message = format!("{}, {}", acc.message, input.id);

            acc
        })
        .await*/

        Ok(Response::new(response))
    }

    type BidirectionalStreamingStream =
        Pin<Box<dyn Stream<Item = Result<WebResponse, Status>> + Send + 'static>>;

    async fn bidirectional_streaming(
        &self,
        request: Request<Streaming<WebRequest>>,
    ) -> Result<Response<Self::BidirectionalStreamingStream>, Status> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(15);
        tokio::spawn(async move {
            let mut stream = request.into_inner();
            while let Some(req) = stream.next().await {
                match req {
                    Ok(result) => {
                        let response = tx
                            .send(Ok(WebResponse {
                                code: 200,
                                message: format!("Hello, {}", result.name),
                            }))
                            .await;
                        if let Err(_e) = response {
                            println!("send err: {}", _e);
                            break;
                        }
                    }
                    Err(e) => {
                        let response = tx
                            .send(Err(Status::new(
                                tonic::Code::Unknown,
                                format!("Error: {}", e),
                            )))
                            .await;

                        if let Err(_e) = response {
                            println!("send err: {}", _e);
                            break;
                        }
                    }
                }
            }
        });

        Ok(Response::new(Box::pin(async_stream::try_stream! {
            while let Some(response) = rx.recv().await {
                yield response?;
            }
        })
            as Self::BidirectionalStreamingStream))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080".parse().unwrap();

    Server::builder()
        .add_service(VotingServer::new(VotingService::default()))
        .add_service(GreeterServer::new(GreetService::default()))
        .add_service(WebServer::new(WebService::default()))
        .serve(address)
        .await?;

    Ok(())
}

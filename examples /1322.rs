use std::{
    io::{Error, ErrorKind, Result},
    net::{Ipv4Addr, Ipv6Addr},
};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
    time::timeout,
};

pub trait CallbackBack<Args = (usize, Vec<u8>)> {
    fn name(&self) -> &str;
    fn init(&mut self);
    fn header_len(&self) -> usize;
    fn protocol_len(&self) -> usize;
    fn callback(&self, args: Args);
}

pub trait ReadWrite<Args = (usize, Vec<u8>)>: CallbackBack<Args> {
    fn read(&mut self) -> Result<()>;
    fn write(&self, protocol: usize, data: &[u8]) -> Result<()>;
}

pub struct TcpClient<T> {
    pub addr: &'static str,
    pub closed: Arc<Mutex<bool>>,
    pub conn: Option<Arc<Mutex<TcpStream>>>,
    pub router: Arc<T>,
}

const MAX_BUFF_SIZE: usize = 8192;

impl<T> TcpClient<T>
    where
        T: ReadWrite + Send + Sync,
        T: Debug + 'static,
{
    fn is_valid_addr(addr: &str) -> Result<()> {
        if let Ok(_) = addr.parse::<Ipv4Addr>() {
            return Ok(());
        }

        if let Ok(_) = addr.parse::<Ipv6Addr>() {
            return Ok(());
        }

        return Err(Error::new(ErrorKind::Other, "not addr "));
    }

    pub async fn connect(&mut self, addr: &'static str) -> Result<()> {
        if let Err(_) = Self::is_valid_addr(addr) {
            return Ok(());
        }

        let mut closed = self.closed.lock().await;
        *closed = true;
        drop(closed); // 释放锁

        let conn = timeout(Duration::from_secs(5), TcpStream::connect(addr)).await??;
        self.conn = Some(Arc::new(Mutex::new(conn)));
        let mut closed = self.closed.lock().await;
        *closed = false;

        Ok(())
    }

    pub async fn new(addr: &'static str, router: T) -> Result<Self> {
        let mut tcp_client = TcpClient {
            addr: addr,
            closed: Arc::new(Default::default()),
            conn: None,
            router: Arc::new(router),
        };

        tcp_client.connect(addr).await?;

        Ok(tcp_client)
    }

    async fn read(&mut self) -> Result<()> {
        if self.conn.is_none() {
            return Ok(());
        }

        let core = self.router.clone();
        eprintln!(
            "tcp_client({}) start listening on {}",
            core.name(),
            &self.addr
        );

        let h_len = core.header_len();
        let p_len = core.protocol_len();
        let conn = self.conn.clone().unwrap();
        tokio::spawn(async move {
            let mut conn = conn.lock().await;
            let mut buff: Vec<u8> = Vec::with_capacity(MAX_BUFF_SIZE);
            let mut hd_buff: Vec<u8> = Vec::with_capacity(h_len);
            let mut protocol: usize = 0;
            let mut body_len: usize = 0;
            let mut buff_len: usize = 0;

            loop {
                let n = conn.read_exact(&mut buff[buff_len..]).await.unwrap();
                if n == 0 {
                    break;
                }
                buff_len += n;

                if body_len == 0 && buff.len() >= h_len {
                    hd_buff.copy_from_slice(&buff[..h_len]);
                    protocol = match p_len {
                        2 => u16::from_be_bytes(hd_buff.clone().try_into().unwrap()) as usize,
                        4 => u32::from_be_bytes(hd_buff.clone().try_into().unwrap()) as usize,
                        8 => u64::from_be_bytes(hd_buff.clone().try_into().unwrap()) as usize,
                        _ => 2usize,
                    };

                    let others = &buff[h_len - p_len..h_len];
                    body_len = match h_len - p_len {
                        2 => u16::from_be_bytes(others.try_into().unwrap()) as usize,
                        4 => u32::from_be_bytes(others.try_into().unwrap()) as usize,
                        8 => u64::from_be_bytes(others.try_into().unwrap()) as usize,
                        _ => 2usize,
                    };

                    // 清除已读的头部内容
                    buff.copy_within((buff_len - h_len)..buff_len, 0);
                    buff_len -= h_len;
                }

                if buff_len >= body_len {
                    core.callback((protocol, buff[..body_len].to_owned()));

                    buff.copy_within((buff_len - body_len)..buff_len, 0);
                    buff_len -= body_len;
                    protocol = 0;
                    body_len = 0;
                }
            }
        });

        Ok::<(), Error>(())
    }

    async fn write(&self, protocol: usize, data: &[u8]) -> Result<()> {
        if self.conn.is_none() {
            let closed = self.closed.lock().await;
            if !(*closed) {
                return Ok(());
            }
        }

        let h_len = self.router.header_len();
        let p_len = self.router.protocol_len();
        let mut buffer = Vec::with_capacity(h_len + data.len());
        match p_len {
            8 => buffer.write_u64(protocol as u64).await?,
            4 => buffer.write_u32(protocol as u32).await?,
            2 | _ => buffer.write_u16(protocol as u16).await?,
        }
        match h_len - p_len {
            8 => buffer.write_u64(data.len() as u64).await?,
            4 => buffer.write_u32(data.len() as u32).await?,
            2 | _ => buffer.write_u16(data.len() as u16).await?,
        }

        buffer.extend_from_slice(&data[..]);
        let conn = self.conn.clone().unwrap();
        let mut conn_lock = conn.lock().await;
        Ok(conn_lock.write_all(&buffer[..]).await?)
    }
}

fn main() {
    println!("callback success...");
}

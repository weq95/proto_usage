use std::time::Duration;
use std::{
    io::{Error, ErrorKind, Result},
    net::{ Ipv4Addr, Ipv6Addr},
};
use std::{sync::Arc};

use byteorder::{LittleEndian, WriteBytesExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{
    net::{TcpStream},
    sync::Mutex,
    time::timeout,
};

pub trait Callback<Args = (usize, Vec<u8>)> {
    fn name(&self) -> &str;
    fn init(&mut self);
    fn header_len(&self) -> usize;
    fn protocol_len(&self) -> usize;
    fn callback(&self, args: Args);
}

#[derive(Clone)]
pub struct TcpClient<T: Callback + Send + Sync + 'static> {
    pub addr: String,
    pub closed: Arc<Mutex<bool>>,
    pub conn: Option<Arc<Mutex<TcpStream>>>,
    pub router: Arc<T>,
}

const MAX_BUFF_SIZE: usize = 8192;

impl<T: Callback + Send + Sync + 'static> TcpClient<T> {
    pub async fn read(&mut self) -> Result<()> {
        if self.conn.is_none() {
            return Ok(());
        }

        let core = self.router.clone();
        eprintln!(
            "TcpServer({}) start listening on {}",
            core.name(),
            &self.addr
        );

        let h_len = core.header_len();
        let p_len = core.protocol_len();
        let conn = self.conn.clone().unwrap();
        let core = core.clone();
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
                } // 链接已关闭

                buff_len += n;
                // read header
                if body_len == 0 && buff.len() >= h_len {
                    hd_buff.copy_from_slice(&buff[..h_len]);
                    protocol = match p_len {
                        2 => u16::from_be_bytes(hd_buff.clone().try_into().unwrap()) as usize,
                        4 => u32::from_be_bytes(hd_buff.clone().try_into().unwrap()) as usize,
                        8 => u64::from_be_bytes(hd_buff.clone().try_into().unwrap()) as usize,
                        _ => 2,
                    };
                    let others = &buff[h_len - p_len..h_len];
                    body_len = match h_len - p_len {
                        2 => u16::from_be_bytes(others.try_into().unwrap()) as usize,
                        4 => u32::from_be_bytes(others.try_into().unwrap()) as usize,
                        8 => u64::from_be_bytes(others.try_into().unwrap()) as usize,
                        _ => 2,
                    };

                    // 清除已解析的头部头部内容
                    buff.copy_within((buff_len - h_len)..buff_len, 0);
                    buff_len -= h_len;
                }

                // read body
                if buff_len >= body_len {
                    core.callback((protocol, buff[..body_len].to_owned()));

                    // 清除一读的body内容
                    buff.copy_within((buff_len - body_len)..buff_len, 0);
                    buff_len -= body_len;
                    protocol = 0;
                    body_len = 0;
                }
            }
        });

        Ok::<(), Error>(())
    }

    // 检测是否是合法的IP地址
    fn is_valid_addr(addr: &str) -> Result<()> {
        if let Ok(_) = addr.parse::<Ipv4Addr>() {
            return Ok(());
        }

        if let Ok(_) = addr.parse::<Ipv6Addr>() {
            return Ok(());
        }

        return Err(Error::new(ErrorKind::Other, "not addr "));
    }

    pub async fn new(addr: &str, router: T) -> Result<Self> {
        let mut tcp_server = TcpClient {
            addr: addr.to_string(),
            closed: Arc::new(Default::default()),
            conn: None,
            router: Arc::new(router),
        };

        tcp_server.connect(addr).await?;

        Ok(tcp_server)
    }

    pub async fn connect(&mut self, addr: &str) -> Result<()> {
        if let Err(_) = Self::is_valid_addr(addr) {
            return Ok(());
        }
        let mut closed = self.closed.lock().await;
        *closed = true;
        let conn = timeout(Duration::from_secs(5), TcpStream::connect(addr)).await??;
        self.conn = Some(Arc::new(Mutex::new(conn)));
        let mut closed = self.closed.lock().await;
        *closed = false;

        Ok(())
    }

    async fn write(&self, protocol: usize, data: &[u8]) -> Result<()> {
        if self.conn.is_none() || self.closed.lock().await.clone() {
            return Err(Error::new(ErrorKind::Other, "server not init"));
        }

        let header_len = self.router.header_len();
        let protocol_len = self.router.protocol_len();
        let mut buffer = Vec::with_capacity(header_len + data.len());
        match protocol_len {
            2 => WriteBytesExt::write_u16::<LittleEndian>(&mut buffer, protocol as u16)?,
            4 => WriteBytesExt::write_u32::<LittleEndian>(&mut buffer, protocol as u32)?,
            8 => WriteBytesExt::write_u64::<LittleEndian>(&mut buffer, protocol as u64)?,
            _ => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("protocol_len = {} 设置错误", protocol_len),
                ));
            }
        }
        match header_len - protocol_len {
            2 => WriteBytesExt::write_u16::<LittleEndian>(&mut buffer, data.len() as u16)?,
            4 => WriteBytesExt::write_u32::<LittleEndian>(&mut buffer, data.len() as u32)?,
            8 => WriteBytesExt::write_u64::<LittleEndian>(&mut buffer, data.len() as u64)?,
            _ => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("content_len = {} 设置错误", header_len - protocol_len),
                ));
            }
        }

        buffer.extend_from_slice(data);
        let conn = self.conn.clone().unwrap();
        let mut conn_lock = conn.lock().await;
        Ok(conn_lock.write_all(&buffer).await?)
    }
}

fn main() {

}
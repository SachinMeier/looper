use postgres::{Client, Error, NoTls};

pub struct Config {
    pub host: String,
    pub port: String,
    pub user: String,
    pub pass: String,
    pub name: String,
}

fn get_addr(cfg: &Config) -> String {
    return format!("{}:{}", cfg.host, cfg.port);
}

pub fn load_config() -> Result<Config, Error> {
    // TODO(sachin) don't unwrap
    let host = std::env::var("POSTGRES_HOST").unwrap();
    let port = std::env::var("POSTGRES_PORT").unwrap();
    let user = std::env::var("POSTGRES_USER").unwrap();
    let pass = std::env::var("POSTGRES_PASS").unwrap();
    let name = std::env::var("POSTGRES_DATABASE").unwrap();
    return Ok(Config{
        host: host,
        port: port,
        user: user,
        pass: pass,
        name: name,
    });
}

pub fn connect(cfg: Config) -> Result<Client, Error> {
    let connstr = format!("postgresql://{}:{}@{}/{}", cfg.user, cfg.pass, get_addr(&cfg), cfg.name);
    let mut client = Client::connect(&connstr, NoTls)?;
    return Ok(client);
}
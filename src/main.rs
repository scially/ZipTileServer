use zip::ZipArchive;
use anyhow::Result;
use std::{fs, fs::File, io::Read, path::Path, collections::HashMap};
use log::info;
use serde::{Serialize, Deserialize};
use actix_web::{get, web, App, HttpResponse, HttpServer};
use std::sync::Arc;
use std::sync::Mutex;
use actix_cors::Cors;
use rusqlite::params;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

trait TileReader {
    fn read(&mut self, path: &str) -> Result<Vec<u8>>;
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Config {
    path: String,
    port: u16,
    host: String,
}

#[derive(Debug)]
struct ZipTile{
    path: String,
    file_name: String,
    zip_reader: zip::ZipArchive<File>
}

#[derive(Debug)]
struct PakTile{
    path: String,
    file_name: String,
    pak_reader: r2d2::Pool<SqliteConnectionManager>
}

impl ZipTile {
    fn new(path: &str) -> Result<ZipTile>{
        let p = Path::new(path);
        let file_name = match p.file_stem().unwrap().to_os_string().into_string(){
            Ok(s) => s,
            Err(e) => {
                return Err(anyhow::anyhow!("can't convert file name  to string"));
            }
        };
    
        let reader = File::open(path)?;
        let zip_reader = ZipArchive::new(reader)?;
        Ok(ZipTile {
            path: path.into(),
            file_name,
            zip_reader
        })
    }
}

impl TileReader for ZipTile{
    fn read(&mut self, file_path: &str) -> Result<Vec<u8>> {
        let mut zip_file = self.zip_reader.by_name(file_path)?;

        let mut buffer = Vec::new();
        zip_file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
}

impl PakTile {
    fn new(path: &str) -> Result<PakTile>{
        let p = Path::new(path);

        if !p.exists() {
            return Err(anyhow::anyhow!("{} not exist", path));
        };

        let file_name = match p.file_stem().unwrap().to_os_string().into_string(){
            Ok(s) => s,
            Err(e) => {
                return Err(anyhow::anyhow!("can't convert file name  to string"));
            }
        };
    
        let manager = SqliteConnectionManager::file(p);
        let pool = Pool::new(manager)?;
        Ok(PakTile {
            path: path.into(),
            file_name,
            pak_reader: pool
        })
    }

    fn read_tms(&mut self, x: i32, y: i32, z: i32) -> Result<Vec<u8>>{
        let conn = self.pak_reader.get()?;
        let blocks = match z {
            0..=9 => "blocks".into(),
            _ => {
                format!("blocks_{}_{}_{}", z, x/512, y/512)
            }
        };
        let query_sql = format!("select `tile` from {} where `x` = ? and `y` = ? and `z` = ?", &blocks);

        let mut stmt = conn.prepare(&query_sql)?;

        let res = stmt.query_row(params![&x, &y, &z], |row| {
            let buffer: Vec<u8> = row.get(0)?;
            Ok(buffer)
        })?;
        return Ok(res);
    }

    fn read_xyz(&mut self, x: i32, y: i32, z: i32) -> Result<Vec<u8>>{
        let y_tms = 2i32.pow(z as u32) - 1 - y;
        self.read_tms(x, y_tms, z)
    }
}

impl TileReader for PakTile {
    fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        let conn = self.pak_reader.get()?;
        if let Some(_) = path.find("tilemapresource.xml"){
            let mut stmt = conn.prepare("select tmsxml from infos")?;
            let res = stmt.query_row(params![], |row| {
                let buffer: Vec<u8> = row.get(0)?;
                Ok(buffer)
            })?;
            return Ok(res);
        }
        let bound = path.len() - 4;
        if let Some(tile_path) = path.get(..bound){
            let tile_coors: Vec<&str> = tile_path.split("/").collect();
            if tile_coors.len() != 4 {
                return Err(anyhow::anyhow!("path format is not right"));
            }
            let z: i32 = tile_coors[1].parse()?;
            let x: i32 = tile_coors[2].parse()?;
            let y: i32 = tile_coors[3].parse()?;

            let res = match tile_coors[0] {
                "tms" => self.read_tms(x, y, z),
                "xyz" => self.read_xyz(x, y, z),
                _ => Err(anyhow::anyhow!("tile format must be xyz or tms"))
            };
            return res;
        };
        Err(anyhow::anyhow!("can't parse tile request path"))
    }
}

type TileReaderType = Arc<Mutex<HashMap<String, Box<dyn TileReader + Send>>>>;

#[get("/tile/{tile_name}/{tail:.*}")]
async fn tile_server(data: web::Data<TileReaderType>, path: web::Path<(String, String)>) -> HttpResponse  {
    if let Ok(mut tile_readers) = data.lock(){
        let (tile_name, tile_path) = path.into_inner();
        if let Some(tile_reader) = tile_readers.get_mut(&tile_name){
            if let Ok(buffer) = tile_reader.read(&tile_path){
                let content_type: &str;
                if  tile_path.ends_with("json"){
                    content_type = "application/json";
                }
                else if tile_path.ends_with("xml"){
                    content_type = "application/xml";
                }
                else if tile_path.ends_with("jpg"){
                    content_type = "image/jpg";
                }
                else if tile_path.ends_with("png"){
                    content_type = "image/png";
                }
                else{
                    content_type = "application/octet-stream";
                }
                return HttpResponse::Ok()
                    .content_type(content_type)
                    .body(buffer);
            }
        }
    }
    HttpResponse::NotFound().finish()
}

#[actix_web::main]
async fn main() ->Result<()> {
    let env = env_logger::Env::default()
                            .filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::Builder::from_env(env) 
        .init();

    let config: Config;
    match fs::read_to_string("config.yaml"){
        Ok(s) => {
            config = serde_yaml::from_str(&s)?;
        },
        Err(_) => {
            config = Config {
                path: "./tiles".into(),
                port: 8080,
                host: "127.0.0.1".into(),
            };
        }
    };
    
    let entrys =   fs::read_dir(config.path)?;
    let tiles: TileReaderType = Arc::new(Mutex::new(HashMap::new()));

    for entry in entrys {
        if let Ok(path) = entry {
            let path = path.path();
            if let Some(ext) = path.extension() {
                if path.is_file() && ext == "zip" {
                    if let Ok(tile) = ZipTile::new(path.to_str().unwrap()){
                        tiles.lock().unwrap().insert(tile.file_name.clone(), Box::new(tile));
                    }
                }

                if path.is_file() && ext == "pak" {
                    if let Ok(tile) = PakTile::new(path.to_str().unwrap()){
                        tiles.lock().unwrap().insert(tile.file_name.clone(), Box::new(tile));
                    }
                }
            }
        }
    }

    info!("ZipTileServer listen on {}:{}", config.host, config.port);
    let res = HttpServer::new(move|| {
        let cors = Cors::default()
              .send_wildcard()
              .allow_any_header()
              .allow_any_method()
              .allow_any_origin()
              .max_age(3600);

        App::new()
            .app_data(web::Data::new(tiles.clone()))
            .wrap(cors)
            .service(tile_server)        
    })
    .bind(("127.0.0.1", config.port))?
    .run()
    .await;
   
    Ok(())
}
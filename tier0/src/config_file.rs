use std::fs::File;
use std::io::{Read, Write};
use std::num::ParseIntError;
use std::ops::{Add, Deref};
use std::path::Path;
use log::debug;

type IoResult = std::io::Result<()>;

#[derive(Clone)]
pub enum Pair {
    Float { key: String, value: f64 },
    Int { key: String, value: i64 },
    String { key: String, value: String },
    Color { key: String, value: Vec<i8> },
    Empty { key: String }
}

impl Pair {
	pub fn float( &self ) -> Option<f64> {
		match self {
			Pair::Float { key: _key, value } => Some( value.clone() ),
			_ => None
		}
	}
	pub fn integer( &self ) -> Option<i64> {
		match self {
			Pair::Int { key: _key, value } => Some( value.clone() ),
			_ => None
		}
	}
	pub fn string( &self ) -> Option<String> {
		match self {
			Pair::String { key: _key, value } => Some( value.clone() ),
			_ => None
		}
	}
	pub fn color( &self ) -> Option<Vec<i8>> {
		match self {
			Pair::Color { key: _key, value } => Some( value.clone() ),
			_ => None
		}
	}
}


pub struct ConfigFile {
    items: Vec<Pair>,
    path: Option<String>
}

impl ConfigFile {
    pub fn new( path: &Path ) -> ConfigFile {
        assert!( path.exists() );
        let mut file: String = String::new();
        let size = File::open( path ).unwrap().read_to_string(&mut file );
	    if size.is_err() {
	        panic!( "WTF! {}", size.err().unwrap() );
        }

        let mut config = ConfigFile { items: Vec::new(), path: Some( path.to_str().unwrap().to_string() ) };

        for line in file.split( "\n" ) {
	        // skip empty lines
	        if line.is_empty() {
		        continue;
	        }

            let pair: Vec<&str> = line.split(" ").collect();
            let name: String = String::from( pair.get(0).unwrap().deref().trim() );
            let mut value: String = line.clone().trim().to_string();
	        value.remove_matches( name.clone().add(" ").as_str() );
	        value = value.trim().to_string();


	        debug!( "key: {}, value: {}", name.escape_default(), value.escape_default() );

	        // is it nothing?
	        if value.is_empty() {
		        config.items.push( Pair::Empty { key: name } );
		        continue;
	        }

	        // is it a float?
	        let float_res = value.parse::<f64>();
	        if float_res.is_ok() && value.contains(".") {
				config.items.push( Pair::Float { key: name.clone(), value: float_res.unwrap() } );
		        continue;
	        }

	        // is it an integer?
	        let int_res = value.parse::<i64>();
	        if int_res.is_ok() {
		        config.items.push( Pair::Int { key: name.clone(), value: int_res.unwrap() } );
		        continue;
	        }

	        // is it a color vector?
	        let color_res: Vec<Result<i8, ParseIntError>> = value.split_whitespace()
		        .map( |value| value.parse::<i8>() )
		        .filter( |v| v.is_ok() )
		        .collect();
	        if color_res.len() == 3 || color_res.len() == 4 {
		        config.items.push( Pair::Color {
			        key: name.clone(),
			        value: color_res.iter()
				        .map( |v| v.clone().unwrap() )
				        .map( |v| v.clone() )
				        .collect()
		        } );
		        continue;
	        }

	        // nothing worked, its a string, but was it quoted?
	        if value.starts_with("\"") && value.ends_with("\"") {
		        // it was "quoted", remove the things
		        value.remove( value.len() - 1 );
		        value.remove( 0 );
	        }
	        config.items.push( Pair::String { key: name.clone(), value: value.clone() } );
        }

        return config;
    }

	pub fn save( &self ) -> IoResult {
		let mut file = File::create( Path::new( self.path.clone().unwrap().as_str() ) ).unwrap();
		let mut res;

		for item in &self.items {
			match item {
				Pair::Float { key: _key, value: _value } => {
					res = writeln!( file, "{} {}", _key, _value );
				}
				Pair::Int { key: _key, value: _value } => {
					res = writeln!( file, "{} {}", _key, _value );
				}
				Pair::String { key: _key, value: _value } => {
					res = writeln!( file, "{} {}", _key, _value );
				}
				Pair::Color { key: _key, value: _value } => {
					res = writeln!( file, "{} {} {} {}", _key, _value[0], _value[1], _value[2] );
				}
				Pair::Empty { key: _key } => {
					res = writeln!( file, "{}", _key );
				}
			}
			if res.is_err() {
				return res;
			}
		}
		Ok( () )
	}

	pub fn set_path( &mut self, path: &Path ) {
		self.path = Some( path.to_str().unwrap().to_string() )
	}

	/**
	 * Same as calling set_path() and then save()
	 */
	pub fn save_to( &mut self, path: &Path ) -> IoResult {
		self.set_path( path );
		self.save()
	}

	pub fn iterator( &self ) -> impl Iterator< Item = &Pair > {
		return ( &self.items ).into_iter();
	}

	pub fn get( &self, key: &str ) -> Option< &Pair > {
		for pair in &self.items {
			match pair {
				Pair::Float { key: _key, value: _ } => {
					if _key == key {
						return Some( &pair );
					}
				}
				Pair::Int { key: _key, value: _ } => {
					if _key == key {
						return Some( &pair );
					}
				}
				Pair::String { key: _key, value: _ } => {
					if _key == key {
						return Some( &pair );
					}
				}
				Pair::Color { key: _key, value: _ } => {
					if _key == key {
						return Some( &pair );
					}
				}
				Pair::Empty { key: _key } => {
					if _key == key {
						return Some( &pair );
					}
				}
			}

		}
		return None;
	}

	pub fn set( &mut self, key: &str, value: Pair ) {
		for idx in 0 .. self.items.len() {
			let item = &self.items[idx];
			match item {
				Pair::Float { key: _key, value: _ } => {
					if key == _key {
						self.items[idx] = value.clone();
						return;
					}
				}
				Pair::Int { key: _key, value: _ } => {
					if key == _key {
						self.items[idx] = value.clone();
						return;
					}
				}
				Pair::String { key: _key, value: _ } => {
					if key == _key {
						self.items[idx] = value.clone();
						return;
					}
				}
				Pair::Color { key: _key, value: _ } => {
					if key == _key {
						self.items[idx] = value.clone();
						return;
					}
				}
				Pair::Empty { key: _key } => {
					if key == _key {
						self.items[idx] = value.clone();
						return;
					}
				}
			}
		}
		// there was no Pair with that key yet, create it
		self.items.push( value );
	}
}

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, ErrorKind, Read, Write};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::path::Path;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};

#[derive(Debug)]
pub enum UpkfError {
	NotAnUpkFileError,
	CorruptedDataError,
	VersionNotSupportedError,
	IoError { err: ErrorKind },
	Crc32CheckFailed,
	Sha256CheckFailed,
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub enum CompressionType {
	NONE,
	LZMA,
	LZMA2,
	GZIP,
	BZIP2
}

impl CompressionType {
	pub fn name( &self ) -> &'static str {
		match self {
			CompressionType::NONE => "NONE",
			CompressionType::LZMA => "LZMA",
			CompressionType::LZMA2 => "LZMA2",
			CompressionType::GZIP => "GZIP",
			CompressionType::BZIP2 => "BZIP2"
		}
	}
}

impl Display for &CompressionType {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			CompressionType::NONE => write!( f, "NONE" ),
			CompressionType::LZMA => write!( f, "LZMA" ),
			CompressionType::LZMA2 => write!( f, "LZMA2" ),
			CompressionType::GZIP => write!( f, "GZIP" ),
			CompressionType::BZIP2 => write!( f, "BZIP2" )
		};
		Ok(())
	}
}

impl TryFrom<u8> for CompressionType {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			x if x == CompressionType::NONE as u8 => Ok( CompressionType::NONE ),
			x if x == CompressionType::LZMA as u8 => Ok( CompressionType::LZMA ),
			x if x == CompressionType::LZMA2 as u8 => Ok( CompressionType::LZMA2 ),
			x if x == CompressionType::GZIP as u8 => Ok( CompressionType::GZIP ),
			x if x == CompressionType::BZIP2 as u8 => Ok( CompressionType::BZIP2 ),
			_ => Err(()),
		}
	}
}

impl TryFrom<&str> for CompressionType {
	type Error = ();

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			x if x == "NONE" => Ok( CompressionType::NONE ),
			x if x == "LZMA" => Ok( CompressionType::LZMA ),
			x if x == "LZMA2" => Ok( CompressionType::LZMA2 ),
			x if x == "GZIP" => Ok( CompressionType::GZIP ),
			x if x == "BZIP2" => Ok( CompressionType::BZIP2 ),
			_ => Err(()),
		}
	}
}

pub struct Upkf {
	origin: String,  // origin of the pak
	entries: Vec<Element>  // entries
}

impl Upkf {
	pub fn add_text_file( &mut self, path: String, data: String, compression: CompressionType ) -> &mut Self {
		self.add_file(
			path,
			String::new(),
			Bytes::copy_from_slice( data.as_bytes() ),
			false,
			compression
		)
	}

	pub fn add_binary_file( &mut self, path: String, data: Bytes, compression: CompressionType ) -> &mut Upkf {
		self.add_file(
			path,
			String::new(),
			data,
			true,
			compression
		)
	}

	pub fn add_file( &mut self, path: String, meta: String, data: Bytes, binary: bool, compression: CompressionType ) -> &mut Upkf {
		self.entries.push( Element {
			path: path,
			meta: meta,
			binary: binary,
			compression: compression,
			bytes: data,
			crc32: Option::None,
			sha256: Option::None
		} );
		self
	}

	pub fn new( origin: String ) -> Self {
		Self {
			origin: origin,
			entries: vec![]
		}
	}

	pub fn load( path: &Path, check_content: bool ) -> Self {
		let file = File::open( path ).unwrap();
		let header = FileHeader::load( &file ).unwrap();
		let mut upkf = Self { origin: header.origin, entries: vec![] };

		for _index in 0 .. header.entry_count {
			let entry_header = EntryHeader::load( &file ).unwrap();
			let entry = Entry::load( &file, &entry_header ).unwrap();
			upkf.entries.push( Element::load( entry_header, entry, check_content ).unwrap() );
		}
		upkf
	}

	pub fn save( &self, path: &Path ) -> Result<(), UpkfError> {
		let mut file = File::create( path ).unwrap();
		FileHeader::new( self.origin.clone(), self.entries.len() as u64, false ).save( &file );
		for entry in &self.entries {
			entry.write( &mut file );
		}
		Ok(())
	}

	pub fn get_path( &self ) -> &Path {
		Path::new( "" )
	}

	pub fn get_origin( &self ) -> &String {
		&self.origin
	}

	pub fn iter(&self) -> UpkfIterator {
		UpkfIterator { current_iter_entry: 0, data: &self.entries }
	}
}

pub struct UpkfIterator<'a> {
	current_iter_entry: usize,
	data: &'a Vec<Element>
}

impl<'a> Iterator for UpkfIterator<'a> {
	type Item = &'a Element;

	fn next(&mut self) -> Option<Self::Item> {
		self.current_iter_entry += 1;

		if self.current_iter_entry < self.data.len() {
			Some( self.data.get( self.current_iter_entry).unwrap() )
		} else {
			None
		}
	}
}

#[derive(Debug, Clone)]
pub struct Element {
	path: String,
	meta: String,
	binary: bool,
	compression: CompressionType,
	bytes: Bytes,
	crc32: Option<u32>,
	sha256: Option<String>,
}

impl Element {
	fn write( &self, file: &mut File ) {
		// compress bytes
		let mut bytes = Cursor::new( vec![] );
		match self.compression {
			CompressionType::NONE => {
				bytes.write( &self.bytes.to_vec() );
			}
			CompressionType::LZMA => {
				lzma_rs::lzma_compress( &mut self.bytes.clone().reader(), &mut bytes );
			}
			CompressionType::LZMA2 => {
				lzma_rs::lzma2_compress( &mut self.bytes.clone().reader(), &mut bytes );
			}
			CompressionType::GZIP => {
				let mut encoder = libflate::gzip::Encoder::new( &mut bytes ).unwrap();
				encoder.write_all( &mut self.bytes.clone().to_vec() );
			}
			CompressionType::BZIP2 => {
				let mut encoder = bzip2::Compress::new( bzip2::Compression::best(), 30 );
				let err = encoder.compress( &mut self.bytes.clone().to_vec(), bytes.get_mut(), bzip2::Action::Run );
				println!( "{:?}, {}, {}", err, encoder.total_in(), encoder.total_out() );
			}
		}
		// calculate sha256
		let sha = sha256::digest_bytes( bytes.get_ref() );
		// create and save header and entry
		EntryHeader {
			size: bytes.get_ref().len() as u64,
			name_size: self.path.as_bytes().len() as u32,
			name: self.path.clone(),
			binary: self.binary,
			compression_type: self.compression,
			crc: crc32fast::hash( bytes.get_ref() ),
			sha256_size: sha.as_bytes().len() as u16,
			sha256: sha,
			metadata_size: self.meta.as_bytes().len() as u32,
			metadata: self.meta.clone()
		}.save( file );
		Entry { data: Bytes::copy_from_slice( &bytes.get_mut().as_mut_slice() ) }.save( file );
	}

	fn load( entry_header: EntryHeader, entry: Entry, check_content: bool ) -> Result<Self, UpkfError> {
		if check_content {
			// check crc and sha256
			if crc32fast::hash( &entry.data ) != entry_header.crc {
				return Result::Err( UpkfError::Crc32CheckFailed );
			}
			if sha256::digest_bytes( &entry.data ) != entry_header.sha256 {
				return Result::Err( UpkfError::Sha256CheckFailed );
			}
		}
		// decompress bytes
		let mut bytes = Cursor::new( vec![] );
		match entry_header.compression_type {
			CompressionType::NONE => {
				bytes = Cursor::new( entry.data.to_vec() );
			}
			CompressionType::LZMA => {
				lzma_rs::lzma_decompress( &mut entry.data.reader(), &mut bytes );
			}
			CompressionType::LZMA2 => {
				lzma_rs::lzma2_decompress(&mut entry.data.reader(), &mut bytes );
			}
			CompressionType::GZIP => {
				let mut decoder = libflate::gzip::Decoder::new( Cursor::new( entry.data.clone() ) ).unwrap();
				decoder.read( bytes.get_mut() );
			}
			CompressionType::BZIP2 => {
				let mut decoder = bzip2::Decompress::new(false);
				let err= decoder.decompress( &mut entry.data.clone().to_vec(), &mut bytes.get_mut() );
				println!( "{:?}", err );
			}
		}
		// create element
		Ok(
			Element {
				path: entry_header.name,
				meta: entry_header.metadata,
				binary: entry_header.binary,
				compression: entry_header.compression_type,
				bytes: Bytes::copy_from_slice( bytes.get_ref().as_slice() ),
				crc32: Option::Some( entry_header.crc ),
				sha256: Option::Some( entry_header.sha256 ),
			}
		)
	}

	pub fn get_path( &self ) -> &String {
		&self.path
	}

	pub fn get_meta( &self ) -> &String {
		&self.meta
	}

	pub fn is_bynary( &self ) -> &bool {
		&self.binary
	}

	pub fn get_content( &self ) -> &Bytes {
		&self.bytes
	}

	pub fn is_compressed( &self ) -> bool {
		self.compression != CompressionType::NONE
	}

	pub fn get_compression( &self ) -> &CompressionType {
		&self.compression
	}

	pub fn get_crc32( &self ) -> &Option<u32> {
		&self.crc32
	}

	pub fn get_sha256( &self ) -> &Option<String> {
		&self.sha256
	}
}

#[derive(Debug)]
struct FileHeader {
	signature: u32,
	version: u8,
	recompressed: bool,
	origin_size: u16,
	origin: String,
	entry_count: u64
}

impl FileHeader {
	fn new(origin: String, entry_count: u64, compress_entries: bool ) -> FileHeader {
		return FileHeader {
			signature: 0x464b5055,
			version: 0,
			recompressed: compress_entries,
			origin_size: origin.as_bytes().len() as u16,
			origin: origin,
			entry_count: entry_count
		}
	}

	fn save( &self, mut file: &File ) -> Result<(), UpkfError> {
		file.write_u32::<LittleEndian>( self.signature );
		file.write_u8( self.version );
		file.write_u8( self.recompressed as u8 );
		file.write_u16::<LittleEndian>( self.origin_size );
		file.write( self.origin.as_bytes() );
		file.write_u64::<LittleEndian>( self.entry_count );
		Ok(())
	}

	fn load(mut file: &File ) -> Result<Self, UpkfError> {
		let signature = file.read_u32::<LittleEndian>().unwrap();
		if signature != 0x464b5055 {
			return Result::Err(UpkfError::NotAnUpkFileError)
		}
		let version = file.read_u8().unwrap();
		if version != 0 {
			return Result::Err(UpkfError::VersionNotSupportedError)
		}
		let recompressed = file.read_u8().unwrap() != 0;
		let origin_size = file.read_u16::<LittleEndian>().unwrap();
		let mut buf = vec![ 0 as u8; origin_size as usize ];
		file.read_exact( &mut buf );
		let origin = String::from_utf8( buf ).unwrap();
		let entry_count = file.read_u64::<LittleEndian>().unwrap();
		return Ok(
			FileHeader {
				signature: signature,
				version: version,
				recompressed: recompressed,
				origin_size: origin_size,
				origin: origin,
				entry_count: entry_count
			}
		)
	}
}

struct EntryHeader {
	size: u64,
	name_size: u32,
	name: String,
	binary: bool,
	compression_type: CompressionType,
	crc: u32,
	sha256_size: u16,
	sha256: String,
	metadata_size: u32,
	metadata: String
}

impl EntryHeader {
	fn save( &self, mut file: &File ) -> Result<(), UpkfError> {
		file.write_u64::<LittleEndian>( self.size );
		file.write_u32::<LittleEndian>( self.name_size );
		file.write(self.name.as_bytes() );
		file.write_u8(self.binary as u8 );
		file.write_u8(self.compression_type as u8 );
		file.write_u32::<LittleEndian>(self.crc );
		file.write_u16::<LittleEndian>(self.sha256.as_bytes().len() as u16 );
		file.write(self.sha256.as_bytes() );
		file.write_u32::<LittleEndian>(self.metadata_size );
		file.write(self.metadata.as_bytes() );
		Ok(())
	}

	fn load( mut file: &File ) -> Result<Self, UpkfError> {
		let size = file.read_u64::<LittleEndian>().unwrap();
		let name_size = file.read_u32::<LittleEndian>().unwrap();
		let mut name_buf = vec![ 0 as u8; name_size as usize ]; file.read_exact( &mut name_buf );
		let name = String::from_utf8(name_buf).unwrap();
		let binary = file.read_u8().unwrap() != 0;
		let compression_type = CompressionType::try_from( file.read_u8().unwrap() ).unwrap_or(CompressionType::NONE);
		let crc = file.read_u32::<LittleEndian>().unwrap();
		let sha256_size = file.read_u16::<LittleEndian>().unwrap();
		let mut sha256_buf = vec![ 0 as u8; sha256_size as usize ]; file.read_exact( &mut sha256_buf );
		let sha256 = String::from_utf8( sha256_buf ).unwrap();
		let metadata_size = file.read_u32::<LittleEndian>().unwrap();
		let mut metadata_buf = vec![ 0 as u8; metadata_size as usize ]; file.read_exact( &mut metadata_buf);
		let metadata = String::from_utf8(metadata_buf).unwrap();
		return Ok(
			EntryHeader {
				size: size,
				name_size: name_size,
				name: name,
				binary: binary,
				compression_type: compression_type,
				crc: crc,
				sha256_size: sha256_size,
				sha256: sha256,
				metadata_size: metadata_size,
				metadata: metadata
			}
		)
	}
}

struct Entry {
	data: Bytes
}

impl Entry {
	fn save( &self, mut file: &File ) -> Result<(), UpkfError> {
		file.write( self.data.deref() );
		Ok(())
	}

	fn load(mut file: &File, header: &EntryHeader) -> Result<Self, UpkfError> {
		let mut buf = vec![1 as u8; header.size as usize ];
		file.read_exact( &mut buf );
		return Ok(
			Entry {
				data: Bytes::from( buf )
			}
		)
	}
}

pub struct UpkfMeta {
	compression: CompressionType,
	string_meta: String,
	binary: bool
}

impl UpkfMeta {
	pub fn default( compression: CompressionType ) -> UpkfMeta {
		UpkfMeta {
			compression: compression,
			string_meta: String::new(),
			binary: true
		}
	}

	pub fn get_string_meta( &self ) -> String {
		self.string_meta.clone()
	}

	pub fn is_binary( &self ) -> bool {
		self.binary
	}

	pub fn get_compression( &self ) -> CompressionType {
		self.compression
	}

	pub fn serialize( &self, file: &mut File ) {
		let mut map: HashMap<&str, &str> = HashMap::new();
		map.insert( "compression", self.compression.name() );
		map.insert( "metadata", &self.string_meta );
		let binary = self.binary.to_string();
		map.insert( "binary", &binary );

		file.write_all( json::stringify_pretty( map, 4 ).as_bytes() );
	}

	pub fn deserialize( file: &mut File, default_compression: CompressionType ) -> Result<UpkfMeta, ()> {
		let mut src = String::new();
		file.read_to_string( &mut src );
		let value = json::parse(src.as_str() );
		let mut result = UpkfMeta::default( default_compression );
		if value.is_ok() {
			let unwrapped = value.unwrap();
			// compression
			if unwrapped["compression"].is_number() {
				result.compression = CompressionType::try_from( unwrapped["compression"].as_u8().unwrap() ).unwrap_or( default_compression );
			} else if unwrapped["compression"].is_string() {
				result.compression = CompressionType::try_from( unwrapped["compression"].as_str().unwrap() ).unwrap_or( default_compression );
			} else if !unwrapped["compression"].is_null() {
				eprintln!("\t\t- Invalid value for key \"compression\": expected one of LZMA, LZMA2, GZIP, BZIP2, 0, 1, 2, 3 got {}", unwrapped["compression"].dump() );
			}
			// binary
			if unwrapped["binary"].is_boolean() {
				result.binary = unwrapped["binary"].as_bool().unwrap_or( result.binary );
			} else if !unwrapped["binary"].is_null() {
				eprintln!("\t\t- Invalid value for key \"binary\": expected one of true, false got {}", unwrapped["binary"].dump() )
			}
			// metadata
			if unwrapped["metadata"].is_object() {
				result.string_meta = unwrapped["metadata"].dump();
			} else if !unwrapped["metadata"].is_null() {
				eprintln!("\t\t- Invalid type for key \"metadata\": expected object got {}", unwrapped["metadata"] )
			}
			return Result::Ok( result );
		}
		Result::Err(())
	}
}

pub fn main() {
	let upkf = Upkf::new( "UngineTest".to_string() );
	upkf.save( Path::new("./test.upkf") );
	Upkf::load( Path::new("./test.upkf"), false );
}

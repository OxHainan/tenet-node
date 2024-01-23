use ethereum::{EnvelopedDecodable, EnvelopedDecoderError, EnvelopedEncodable};
use ethereum_types::{Bloom, H160, H256, U256};
use rlp::{Decodable, DecoderError, Encodable, Rlp};
type Bytes = alloc::vec::Vec<u8>;
use bytes::BytesMut;
use hex_literal::hex;
use sp_std::vec::Vec;
pub const NON_PERCEPTIBLE: H256 = H256(hex!(
	"b142c3cfa9b0930e084b17693e39e134b43acaa477193956ecad5597db3919a3"
));

pub const PERCEPTIBLE: H256 = H256(hex!(
	"192fdc83f800bf02ba50fdfef52ea118864d5db473845146c64ab757238ef9e2"
));

pub type EIP2930ReceiptData = EIP658ReceiptData;

pub type EIP1559ReceiptData = EIP658ReceiptData;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
	feature = "with-codec",
	derive(scale_codec::Encode, scale_codec::Decode, scale_info::TypeInfo)
)]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Receipt {
	/// EIP658 receipt type
	Legacy(EIP658ReceiptData),
	/// EIP-2930 receipt type
	EIP2930(EIP2930ReceiptData),
	/// EIP-1559 receipt type
	EIP1559(EIP1559ReceiptData),
}

impl EnvelopedEncodable for Receipt {
	fn type_id(&self) -> Option<u8> {
		match self {
			Self::Legacy(_) => None,
			Self::EIP2930(_) => Some(1),
			Self::EIP1559(_) => Some(2),
		}
	}

	fn encode_payload(&self) -> BytesMut {
		match self {
			Self::Legacy(r) => PrivacyEncode::encode(r),
			Self::EIP2930(r) => PrivacyEncode::encode(r),
			Self::EIP1559(r) => PrivacyEncode::encode(r),
		}
	}
}

impl EnvelopedDecodable for Receipt {
	type PayloadDecoderError = rlp::DecoderError;
	fn decode(
		bytes: &[u8],
	) -> Result<Self, ethereum::EnvelopedDecoderError<Self::PayloadDecoderError>> {
		if bytes.is_empty() {
			return Err(EnvelopedDecoderError::UnknownTypeId);
		}

		let first = bytes[0];

		let rlp = Rlp::new(bytes);
		if rlp.is_list() {
			return Ok(Self::Legacy(Decodable::decode(&rlp)?));
		}

		let s = &bytes[1..];
		if first == 0x01 {
			return Ok(Self::EIP2930(rlp::decode(s)?));
		}

		if first == 0x02 {
			return Ok(Self::EIP1559(rlp::decode(s)?));
		}

		Err(DecoderError::Custom("invalid receipt type").into())
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(rlp::RlpDecodable, rlp::RlpEncodable)]
#[cfg_attr(
	feature = "with-codec",
	derive(scale_codec::Encode, scale_codec::Decode, scale_info::TypeInfo)
)]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EIP658ReceiptData {
	pub status_code: u8,
	pub used_gas: U256,
	pub logs_bloom: Bloom,
	pub logs: Vec<Log>,
}

impl PrivacyEncode for EIP1559ReceiptData {
	fn encode(&self) -> BytesMut {
		let logs_root = ethereum::util::ordered_trie_root(self.logs.iter().map(rlp::encode));
		let mut s = rlp::RlpStream::new_list(4);
		println!("receipt root: {:?}", logs_root);
		s.append(&self.status_code);
		s.append(&self.used_gas);
		s.append(&self.logs_bloom);
		s.append(&logs_root);
		s.out()
	}
}

pub trait PrivacyEncode {
	fn encode(&self) -> BytesMut;
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
	feature = "with-codec",
	derive(scale_codec::Encode, scale_codec::Decode, scale_info::TypeInfo)
)]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LogType {
	Perceptible,
	NonPerceptible,
}

impl Encodable for LogType {
	fn rlp_append(&self, s: &mut rlp::RlpStream) {
		match self {
			Self::Perceptible => s.encoder().encode_value(&[0x1]),
			Self::NonPerceptible => s.encoder().encode_value(&[0x2]),
		}
	}
}

impl Decodable for LogType {
	fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
		let val: u8 = rlp.as_val()?;

		if val == 0x1 {
			Ok(Self::Perceptible)
		} else {
			Ok(Self::NonPerceptible)
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
	feature = "with-codec",
	derive(scale_codec::Encode, scale_codec::Decode, scale_info::TypeInfo)
)]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Log {
	pub address: H160,
	pub topics: Vec<H256>,
	pub data: Bytes,
	pub log_type: Option<LogType>,
	pub receivers_root: Option<H256>,
}

impl Encodable for Log {
	fn rlp_append(&self, s: &mut rlp::RlpStream) {
		if self.receivers_root.is_some() {
			s.begin_list(5);
		} else if self.log_type.is_some() {
			s.begin_list(4);
		} else {
			s.begin_list(3);
		}

		s.append(&self.address);
		s.append_list(&self.topics);
		s.append(&self.data);

		if let Some(log_type) = &self.log_type {
			s.append(log_type);
		}

		if let Some(root) = &self.receivers_root {
			s.append(root);
		}
	}
}

impl Decodable for Log {
	fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
		let (log_type, root) = {
			match rlp.item_count()? {
				5 => (Some(rlp.val_at(3)?), Some(rlp.val_at(4)?)),
				4 => (Some(rlp.val_at(3)?), None),
				_ => (None, None),
			}
		};

		Ok(Self {
			address: rlp.val_at(0)?,
			topics: rlp.list_at(1)?,
			data: rlp.val_at(2)?,
			log_type,
			receivers_root: root,
		})
	}
}

impl From<ethereum::Log> for Log {
	fn from(log: ethereum::Log) -> Self {
		let (log_type, receivers_root) = parse_event(&log.topics, &log.data);
		Self {
			address: log.address,
			topics: log.topics,
			data: log.data,
			receivers_root,
			log_type,
		}
	}
}

fn parse_event(topics: &[H256], data: &Bytes) -> (Option<LogType>, Option<H256>) {
	match parse_log_type(topics, data) {
		Some(log_type) => {
			let root = if log_type == LogType::NonPerceptible {
				Some(H256::from_low_u64_be(0xff))
			} else {
				None
			};

			(Some(log_type), root)
		}
		None => (None, None),
	}
}

fn parse_log_type(topics: &[H256], data: &Bytes) -> Option<LogType> {
	let get_id = |data: &Vec<u8>| -> Option<LogType> {
		if data.is_empty() {
			return None;
		}

		let id = H256::from_slice(&data[..32]);
		if id == PERCEPTIBLE {
			Some(LogType::Perceptible)
		} else if id == NON_PERCEPTIBLE {
			Some(LogType::NonPerceptible)
		} else {
			None
		}
	};

	match (parse_log(topics), get_id(data)) {
		(Some(LogType::Perceptible), None) | (None, Some(LogType::Perceptible)) => {
			Some(LogType::Perceptible)
		}
		(Some(LogType::NonPerceptible), None) | (None, Some(LogType::NonPerceptible)) => {
			Some(LogType::NonPerceptible)
		}
		(None, None) => None,
		(Some(_), Some(_)) => None,
	}
}

fn parse_log(topics: &[H256]) -> Option<LogType> {
	if topics[1] == PERCEPTIBLE {
		Some(LogType::Perceptible)
	} else if topics[1] == NON_PERCEPTIBLE {
		Some(LogType::NonPerceptible)
	} else {
		None
	}
}

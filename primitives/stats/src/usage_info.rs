// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Usage statistics

use core::time::{Duration};
use codec::{Encode, Decode};

/// Measured count of operations and total bytes.
#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct UsageUnit {
	/// Number of operations.
	pub ops: u64,
	/// Number of bytes.
	pub bytes: u64,
}

/// Usage statistics for state backend.
#[derive(Clone, Debug, Encode, Decode)]
pub struct UsageInfo {
	/// Read statistics (total).
	pub reads: UsageUnit,
	/// Write statistics.
	pub writes: UsageUnit,
	/// Cache read statistics.
	pub cache_reads: UsageUnit,
	/// Memory used.
	// Encoded as u64 because wasm's usize is u64.
	pub memory: u64,

	/// Moment at which current statistics has been started being collected.
	pub started: Duration,
	/// Timespan of the statistics.
	pub span: Duration,
}

impl UsageInfo {
	/// Empty statistics.
	///
	/// Means no data was collected.
	pub fn empty() -> Self {
		Self {
			reads: UsageUnit::default(),
			writes: UsageUnit::default(),
			cache_reads: UsageUnit::default(),
			memory: 0,
			started: Default::default(),
			span: Duration::new(0, 0),
		}
	}
}
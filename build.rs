#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env::var_os;
use embed_manifest::{embed_manifest, new_manifest, manifest::{HeapType, MaxVersionTested, SupportedOS::Windows10}};

fn main() {
	if var_os("CARGO_CFG_WINDOWS").is_some() {
		embed_manifest(new_manifest("Dusk")
			.supported_os(Windows10..=Windows10)
			.max_version_tested(MaxVersionTested::Windows11Version22H2)
			.heap_type(HeapType::SegmentHeap))
			.expect("unable to embed manifest file");
	}

	println!("cargo:rerun-if-changed=build.rs");
}

# Tree-Bitmap: Fast IP lookup table for IPv4/IPv6 prefixes

This crate provides a datastructure for fast IP address lookups.
It aims at fast lookup times, and a reasonable memory footprint.

The internal datastructure is based on the Tree-bitmap algorithm described by W. Eatherton, Z. Dittia, G. Varghes.

## Documentation

Rustdoc: https://hroi.github.io/treebitmap/

## Requirements

Treebitmap uses ```RawVec``` for its allocation, which requires Rust nightly for now.

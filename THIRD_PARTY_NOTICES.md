# Third-party notices

Xdownload2 is built from the open-source Rust crates listed below. All of them are under
permissive licenses (MIT, Apache-2.0, BSD, Zlib, ISC, Unicode, BSL, Unlicense and similar);
their full license texts are in each crate's source at https://crates.io.

## Programs downloaded at run time (not part of the exe)

| Program | License | Source |
|---|---|---|
| yt-dlp | Unlicense | https://github.com/yt-dlp/yt-dlp |
| FFmpeg (BtbN win64 GPL build) | GPL-3.0-or-later | https://github.com/BtbN/FFmpeg-Builds, https://ffmpeg.org |

Xdownload2 downloads these from their own release pages on first use, verifies the published
SHA-256 and runs them as separate programs. It does not include or link their code.

## Fonts and icons

- Phosphor Icons (MIT) through the `egui-phosphor` crate.
- egui's bundled default fonts (OFL-1.1, Ubuntu Font Licence 1.0). Text is drawn with the
  Segoe UI and Malgun Gothic fonts already installed with Windows; no font file is shipped.

## Rust crates

| Crate | Version | License |
|---|---|---|
| ab_glyph | 0.2.32 | Apache-2.0 |
| ab_glyph_rasterizer | 0.1.10 | Apache-2.0 |
| accesskit | 0.17.1 | MIT OR Apache-2.0 |
| accesskit_consumer | 0.26.0 | MIT OR Apache-2.0 |
| accesskit_windows | 0.24.1 | MIT OR Apache-2.0 |
| accesskit_winit | 0.23.1 | Apache-2.0 |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| ahash | 0.8.12 | MIT OR Apache-2.0 |
| ahash | 0.8.12 | MIT OR Apache-2.0 (*) |
| arboard | 3.7.0 | MIT OR Apache-2.0 |
| arboard | 3.7.0 | MIT OR Apache-2.0 (*) |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| base64ct | 1.8.3 | Apache-2.0 OR MIT |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| bumpalo | 3.20.4 | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT (*) |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| bytes | 1.12.2 | MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| chrono | 0.4.45 | MIT OR Apache-2.0 |
| clipboard-win | 5.4.3 | BSL-1.0 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| crc32fast | 1.5.5 | MIT OR Apache-2.0 |
| crc32fast | 1.5.5 | MIT OR Apache-2.0 (*) |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib |
| der | 0.8.3 | Apache-2.0 OR MIT |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 (*) |
| document-features | 0.2.12 | MIT OR Apache-2.0 |
| dpi | 0.1.2 | Apache-2.0 AND MIT |
| ecolor | 0.31.1 | MIT OR Apache-2.0 |
| eframe | 0.31.1 | MIT OR Apache-2.0 |
| egui | 0.31.1 | MIT OR Apache-2.0 |
| egui | 0.31.1 | MIT OR Apache-2.0 (*) |
| egui-phosphor | 0.9.0 | MIT OR Apache-2.0 |
| egui-winit | 0.31.1 | MIT OR Apache-2.0 |
| egui_glow | 0.31.1 | MIT OR Apache-2.0 |
| emath | 0.31.1 | MIT OR Apache-2.0 |
| emath | 0.31.1 | MIT OR Apache-2.0 (*) |
| epaint | 0.31.1 | MIT OR Apache-2.0 |
| epaint_default_fonts | 0.31.1 | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| error-code | 3.4.0 | BSL-1.0 |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| flate2 | 1.1.11 | MIT OR Apache-2.0 |
| flate2 | 1.1.11 | MIT OR Apache-2.0 (*) |
| foldhash | 0.1.5 | Zlib |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| generic-array | 0.14.7 | MIT |
| generic-array | 0.14.7 | MIT (*) |
| glow | 0.16.0 | MIT OR Apache-2.0 OR Zlib |
| glutin | 0.32.3 | Apache-2.0 |
| glutin | 0.32.3 | Apache-2.0 (*) |
| glutin-winit | 0.5.0 | MIT |
| glutin_egl_sys | 0.7.1 | Apache-2.0 |
| glutin_wgl_sys | 0.6.1 | Apache-2.0 |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 (*) |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| icu_collections | 2.4.0 | Unicode-3.0 |
| icu_collections | 2.4.0 | Unicode-3.0 (*) |
| icu_locale_core | 2.4.0 | Unicode-3.0 |
| icu_locale_core | 2.4.0 | Unicode-3.0 (*) |
| icu_normalizer | 2.4.0 | Unicode-3.0 |
| icu_normalizer_data | 2.4.0 | Unicode-3.0 |
| icu_properties | 2.4.0 | Unicode-3.0 |
| icu_properties_data | 2.4.0 | Unicode-3.0 |
| icu_provider | 2.4.0 | Unicode-3.0 |
| icu_provider | 2.4.0 | Unicode-3.0 (*) |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| image | 0.25.10 | MIT OR Apache-2.0 |
| image | 0.25.10 | MIT OR Apache-2.0 (*) |
| immutable-chunkmap | 2.1.5 | Apache-2.0 OR MIT |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| itoa | 1.0.19 | MIT OR Apache-2.0 |
| libloading | 0.8.9 | ISC |
| litemap | 0.8.3 | Unicode-3.0 |
| litrs | 1.0.1 | MIT OR Apache-2.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.35 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| memoffset | 0.9.1 | MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| native-tls | 0.2.18 | MIT OR Apache-2.0 |
| nohash-hasher | 0.2.0 | Apache-2.0 OR MIT |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| owned_ttf_parser | 0.25.1 | Apache-2.0 |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 (*) |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| paste | 1.0.15 | MIT OR Apache-2.0 |
| pem-rfc7468 | 1.0.0 | Apache-2.0 OR MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.18 | Apache-2.0 OR MIT |
| png | 0.18.1 | MIT OR Apache-2.0 |
| potential_utf | 0.1.6 | Unicode-3.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 (*) |
| profiling | 1.0.18 | MIT OR Apache-2.0 |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 (*) |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| rfd | 0.15.4 | MIT |
| rustls-pki-types | 1.16.0 | MIT OR Apache-2.0 |
| rustls-pki-types | 1.16.0 | MIT OR Apache-2.0 (*) |
| schannel | 0.1.30 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| smallvec | 1.16.6 | MIT OR Apache-2.0 |
| smol_str | 0.2.2 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 (*) |
| syn | 3.0.10 | MIT OR Apache-2.0 |
| syn | 3.0.10 | MIT OR Apache-2.0 (*) |
| synstructure | 0.14.0 | MIT |
| synstructure | 0.14.0 | MIT (*) |
| tinystr | 0.8.5 | Unicode-3.0 |
| tracing | 0.1.44 | MIT |
| tracing-core | 0.1.36 | MIT |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| ureq | 3.5.2 | MIT OR Apache-2.0 |
| ureq-proto | 0.7.1 | MIT OR Apache-2.0 |
| url | 2.5.8 | MIT OR Apache-2.0 |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT (*) |
| utf_types | 1.0.0 | Apache-2.0 OR MIT |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
| webbrowser | 1.2.7 | MIT OR Apache-2.0 |
| webpki-root-certs | 1.0.12 | CDLA-Permissive-2.0 |
| winapi | 0.3.9 | MIT/Apache-2.0 |
| windows | 0.58.0 | MIT OR Apache-2.0 |
| windows-core | 0.58.0 | MIT OR Apache-2.0 |
| windows-core | 0.58.0 | MIT OR Apache-2.0 (*) |
| windows-implement | 0.58.0 | MIT OR Apache-2.0 |
| windows-interface | 0.58.0 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-result | 0.2.0 | MIT OR Apache-2.0 |
| windows-result | 0.2.0 | MIT OR Apache-2.0 (*) |
| windows-strings | 0.1.0 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 (*) |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 (*) |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 (*) |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 (*) |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| winit | 0.30.13 | Apache-2.0 |
| winit | 0.30.13 | Apache-2.0 (*) |
| writeable | 0.6.4 | Unicode-3.0 |
| yoke | 0.8.4 | Unicode-3.0 |
| yoke | 0.8.4 | Unicode-3.0 (*) |
| yoke-derive | 0.8.4 | Unicode-3.0 |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom | 0.1.8 | Unicode-3.0 (*) |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zeroize | 1.9.2 | Apache-2.0 OR MIT |
| zerotrie | 0.2.5 | Unicode-3.0 |
| zerotrie | 0.2.5 | Unicode-3.0 (*) |
| zerovec | 0.11.9 | Unicode-3.0 |
| zerovec | 0.11.9 | Unicode-3.0 (*) |
| zerovec-derive | 0.11.7 | Unicode-3.0 |
| zip | 2.6.1 | MIT |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.4 | Apache-2.0 |

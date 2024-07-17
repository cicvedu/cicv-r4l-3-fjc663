cmd_/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.o := RUST_MODFILE=/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion rustc --edition=2021 -Zbinary_dep_depinfo=y -Dunsafe_op_in_unsafe_fn -Drust_2018_idioms -Dunreachable_pub -Dnon_ascii_idents -Wmissing_docs -Drustdoc::missing_crate_level_docs -Dclippy::correctness -Dclippy::style -Dclippy::suspicious -Dclippy::complexity -Dclippy::perf -Dclippy::let_unit_value -Dclippy::mut_mut -Dclippy::needless_bitwise_bool -Dclippy::needless_continue -Wclippy::dbg_macro --target=./rust/target.json -Cpanic=abort -Cembed-bitcode=n -Clto=n -Cforce-unwind-tables=n -Ccodegen-units=1 -Csymbol-mangling-version=v0 -Crelocation-model=static -Zfunction-sections=n -Dclippy::float_arithmetic -Ctarget-feature=-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx,-avx2 -Ztune-cpu=generic -Cno-redzone=y -Ccode-model=kernel -Copt-level=2 -Cdebug-assertions=n -Coverflow-checks=y -Dwarnings  --cfg MODULE  @./include/generated/rustc_cfg -Zallow-features=allocator_api,bench_black_box,core_ffi_c,generic_associated_types,const_ptr_offset_from,const_refs_to_cell -Zcrate-attr=no_std -Zcrate-attr='feature(allocator_api,bench_black_box,core_ffi_c,generic_associated_types,const_ptr_offset_from,const_refs_to_cell)' --extern alloc --extern kernel --crate-type rlib --out-dir /root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion -L ./rust/ --crate-name completion --emit=dep-info,obj /root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.rs; mv /root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.d /root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/.completion.o.d; sed -i '/^$(pound)/d' /root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/.completion.o.d

source_/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.o := /root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.rs

deps_/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.o := \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libcore.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libcompiler_builtins.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libkernel.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libbindings.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libmacros.so \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/liballoc.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libbuild_error.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libcore.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libcompiler_builtins.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libkernel.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libbindings.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libmacros.so \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/liballoc.rmeta \
  /root/learn/cicv-r4l-3-fjc663/linux/rust/libbuild_error.rmeta \

/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.o: $(deps_/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.o)

$(deps_/root/learn/cicv-r4l-3-fjc663/r4l_experiment/driver/002_completion/completion.o):

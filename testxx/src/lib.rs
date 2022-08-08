#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
use encase::ShaderType;
pub struct CalcText1 {
    m: [f32; 16],
    v: [f32; 16],
    f: f32,
}
const _: fn() = || {
    #[allow(clippy::extra_unused_lifetimes, clippy::missing_const_for_fn)]
    fn check() {
        fn assert_impl<
            T: ?::core::marker::Sized + ::encase::private::ShaderType + ::encase::private::ShaderSize,
        >() {
        }
        assert_impl::<[f32; 16]>();
    }
};
const _: fn() = || {
    #[allow(clippy::extra_unused_lifetimes, clippy::missing_const_for_fn)]
    fn check() {
        fn assert_impl<
            T: ?::core::marker::Sized + ::encase::private::ShaderType + ::encase::private::ShaderSize,
        >() {
        }
        assert_impl::<[f32; 16]>();
    }
};
const _: fn() = || {
    #[allow(clippy::extra_unused_lifetimes, clippy::missing_const_for_fn)]
    fn check() {
        fn assert_impl<
            T: ?::core::marker::Sized + ::encase::private::ShaderType + ::encase::private::ShaderSize,
        >() {
        }
        assert_impl::<f32>();
    }
};
impl ::encase::private::ShaderType for CalcText1
where
    [f32; 16]: ::encase::private::ShaderType + ::encase::private::ShaderSize,
    [f32; 16]: ::encase::private::ShaderType + ::encase::private::ShaderSize,
    f32: ::encase::private::ShaderType,
{
    type ExtraMetadata = ::encase::private::StructMetadata<3usize>;
    const METADATA: ::encase::private::Metadata<Self::ExtraMetadata> = {
        let struct_alignment = ::encase::private::AlignmentValue::max([
            <[f32; 16] as ::encase::private::ShaderType>::METADATA.alignment(),
            <[f32; 16] as ::encase::private::ShaderType>::METADATA.alignment(),
            <f32 as ::encase::private::ShaderType>::METADATA.alignment(),
        ]);
        let extra = {
            let mut paddings = [0; 3usize];
            let mut offsets = [0; 3usize];
            let mut offset = 0;
            offset += <[f32; 16] as ::encase::private::ShaderSize>::SHADER_SIZE.get();
            offsets[1usize] = <[f32; 16] as ::encase::private::ShaderType>::METADATA
                .alignment()
                .round_up(offset);
            let padding = <[f32; 16] as ::encase::private::ShaderType>::METADATA
                .alignment()
                .padding_needed_for(offset);
            offset += padding;
            paddings[0usize] = padding;
            offset += <[f32; 16] as ::encase::private::ShaderSize>::SHADER_SIZE.get();
            offsets[2usize] = <f32 as ::encase::private::ShaderType>::METADATA
                .alignment()
                .round_up(offset);
            let padding = <f32 as ::encase::private::ShaderType>::METADATA
                .alignment()
                .padding_needed_for(offset);
            offset += padding;
            paddings[1usize] = padding;
            offset += <f32 as ::encase::private::ShaderSize>::SHADER_SIZE.get();
            paddings[2usize] = struct_alignment.padding_needed_for(offset);
            ::encase::private::StructMetadata { offsets, paddings }
        };
        let min_size = {
            let mut offset = extra.offsets[3usize - 1];
            offset += <f32 as ::encase::private::ShaderType>::METADATA
                .min_size()
                .get();
            ::encase::private::SizeValue::new(struct_alignment.round_up(offset))
        };
        ::encase::private::Metadata {
            alignment: struct_alignment,
            has_uniform_min_alignment: true,
            min_size,
            extra,
        }
    };
    const UNIFORM_COMPAT_ASSERT: fn() = || {
        ::encase::private::consume_zsts([
            <[f32; 16] as ::encase::private::ShaderType>::UNIFORM_COMPAT_ASSERT(),
            if let ::core::option::Option::Some(min_alignment) =
                <[f32; 16] as ::encase::private::ShaderType>::METADATA.uniform_min_alignment()
            {
                let offset = <Self as ::encase::private::ShaderType>::METADATA.offset(0usize);
                {
                    #[allow(clippy::equatable_if_let)]
                    if let false = min_alignment.is_aligned(offset) {
                        {
                            let mut fmt: ::const_panic::FmtArg = ::const_panic::FmtArg::DEBUG;
                            match &[
                                ::const_panic::StdWrapper(
                                    &match &"offset of field '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"m" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' must be a multiple of " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &min_alignment.get() {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &" (current offset: " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &offset {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &")" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                            ] {
                                args => ::const_panic::concat_panic(args),
                            }
                        }
                    }
                }
            },
            (),
            <[f32; 16] as ::encase::private::ShaderType>::UNIFORM_COMPAT_ASSERT(),
            if let ::core::option::Option::Some(min_alignment) =
                <[f32; 16] as ::encase::private::ShaderType>::METADATA.uniform_min_alignment()
            {
                let offset = <Self as ::encase::private::ShaderType>::METADATA.offset(1usize);
                {
                    #[allow(clippy::equatable_if_let)]
                    if let false = min_alignment.is_aligned(offset) {
                        {
                            let mut fmt: ::const_panic::FmtArg = ::const_panic::FmtArg::DEBUG;
                            match &[
                                ::const_panic::StdWrapper(
                                    &match &"offset of field '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"v" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' must be a multiple of " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &min_alignment.get() {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &" (current offset: " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &offset {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &")" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                            ] {
                                args => ::const_panic::concat_panic(args),
                            }
                        }
                    }
                }
            },
            if let ::core::option::Option::Some(min_alignment) =
                <[f32; 16] as ::encase::private::ShaderType>::METADATA.uniform_min_alignment()
            {
                let prev_offset =
                    <Self as ::encase::private::ShaderType>::METADATA.offset(1usize - 1);
                let offset = <Self as ::encase::private::ShaderType>::METADATA.offset(1usize);
                let diff = offset - prev_offset;
                let prev_size = <[f32; 16] as ::encase::private::ShaderSize>::SHADER_SIZE.get();
                let prev_size = min_alignment.round_up(prev_size);
                {
                    #[allow(clippy::equatable_if_let)]
                    if let false = diff >= prev_size {
                        {
                            let mut fmt: ::const_panic::FmtArg = ::const_panic::FmtArg::DEBUG;
                            match &[
                                ::const_panic::StdWrapper(
                                    &match &"offset between fields '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"m" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' and '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"v" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' must be at least " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &min_alignment.get() {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &" (currently: " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &diff {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &")" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                            ] {
                                args => ::const_panic::concat_panic(args),
                            }
                        }
                    }
                }
            },
            <f32 as ::encase::private::ShaderType>::UNIFORM_COMPAT_ASSERT(),
            if let ::core::option::Option::Some(min_alignment) =
                <f32 as ::encase::private::ShaderType>::METADATA.uniform_min_alignment()
            {
                let offset = <Self as ::encase::private::ShaderType>::METADATA.offset(2usize);
                {
                    #[allow(clippy::equatable_if_let)]
                    if let false = min_alignment.is_aligned(offset) {
                        {
                            let mut fmt: ::const_panic::FmtArg = ::const_panic::FmtArg::DEBUG;
                            match &[
                                ::const_panic::StdWrapper(
                                    &match &"offset of field '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"f" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' must be a multiple of " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &min_alignment.get() {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &" (current offset: " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &offset {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &")" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                            ] {
                                args => ::const_panic::concat_panic(args),
                            }
                        }
                    }
                }
            },
            if let ::core::option::Option::Some(min_alignment) =
                <[f32; 16] as ::encase::private::ShaderType>::METADATA.uniform_min_alignment()
            {
                let prev_offset =
                    <Self as ::encase::private::ShaderType>::METADATA.offset(2usize - 1);
                let offset = <Self as ::encase::private::ShaderType>::METADATA.offset(2usize);
                let diff = offset - prev_offset;
                let prev_size = <[f32; 16] as ::encase::private::ShaderSize>::SHADER_SIZE.get();
                let prev_size = min_alignment.round_up(prev_size);
                {
                    #[allow(clippy::equatable_if_let)]
                    if let false = diff >= prev_size {
                        {
                            let mut fmt: ::const_panic::FmtArg = ::const_panic::FmtArg::DEBUG;
                            match &[
                                ::const_panic::StdWrapper(
                                    &match &"offset between fields '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"v" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' and '" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"f" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &"' must be at least " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &min_alignment.get() {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &" (currently: " {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &diff {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt),
                                )
                                .deref_panic_vals(),
                                ::const_panic::StdWrapper(
                                    &match &")" {
                                        reff => ::const_panic::__::PanicFmt::PROOF
                                            .infer(reff)
                                            .coerce(reff),
                                    }
                                    .to_panicvals(fmt.set_display().set_alternate(false)),
                                )
                                .deref_panic_vals(),
                            ] {
                                args => ::const_panic::concat_panic(args),
                            }
                        }
                    }
                }
            },
        ])
    };
    fn size(&self) -> ::core::num::NonZeroU64 {
        let mut offset = Self::METADATA.last_offset();
        offset += ::encase::private::ShaderType::size(&self.f).get();
        ::encase::private::SizeValue::new(Self::METADATA.alignment().round_up(offset)).0
    }
}
impl ::encase::private::WriteInto for CalcText1
where
    Self: ::encase::private::ShaderType<ExtraMetadata = ::encase::private::StructMetadata<3usize>>,
    [f32; 16]: ::encase::private::WriteInto,
    [f32; 16]: ::encase::private::WriteInto,
    f32: ::encase::private::WriteInto,
{
    fn write_into<B: ::encase::private::BufferMut>(
        &self,
        writer: &mut ::encase::private::Writer<B>,
    ) {
        ::encase::private::WriteInto::write_into(&self.m, writer);
        ::encase::private::Writer::advance(
            writer,
            <Self as ::encase::private::ShaderType>::METADATA.padding(0usize)
                as ::core::primitive::usize,
        );
        ::encase::private::WriteInto::write_into(&self.v, writer);
        ::encase::private::Writer::advance(
            writer,
            <Self as ::encase::private::ShaderType>::METADATA.padding(1usize)
                as ::core::primitive::usize,
        );
        ::encase::private::WriteInto::write_into(&self.f, writer);
        ::encase::private::Writer::advance(
            writer,
            <Self as ::encase::private::ShaderType>::METADATA.padding(2usize)
                as ::core::primitive::usize,
        );
    }
}
impl ::encase::private::ReadFrom for CalcText1
where
    Self: ::encase::private::ShaderType<ExtraMetadata = ::encase::private::StructMetadata<3usize>>,
    [f32; 16]: ::encase::private::ReadFrom,
    [f32; 16]: ::encase::private::ReadFrom,
    f32: ::encase::private::ReadFrom,
{
    fn read_from<B: ::encase::private::BufferRef>(
        &mut self,
        reader: &mut ::encase::private::Reader<B>,
    ) {
        ::encase::private::ReadFrom::read_from(&mut self.m, reader);
        ::encase::private::Reader::advance(
            reader,
            <Self as ::encase::private::ShaderType>::METADATA.padding(0usize)
                as ::core::primitive::usize,
        );
        ::encase::private::ReadFrom::read_from(&mut self.v, reader);
        ::encase::private::Reader::advance(
            reader,
            <Self as ::encase::private::ShaderType>::METADATA.padding(1usize)
                as ::core::primitive::usize,
        );
        ::encase::private::ReadFrom::read_from(&mut self.f, reader);
        ::encase::private::Reader::advance(
            reader,
            <Self as ::encase::private::ShaderType>::METADATA.padding(2usize)
                as ::core::primitive::usize,
        );
    }
}
impl ::encase::private::CreateFrom for CalcText1
where
    Self: ::encase::private::ShaderType<ExtraMetadata = ::encase::private::StructMetadata<3usize>>,
    [f32; 16]: ::encase::private::CreateFrom,
    [f32; 16]: ::encase::private::CreateFrom,
    f32: ::encase::private::CreateFrom,
{
    fn create_from<B: ::encase::private::BufferRef>(
        reader: &mut ::encase::private::Reader<B>,
    ) -> Self {
        let m = ::encase::private::CreateFrom::create_from(reader);
        ::encase::private::Reader::advance(
            reader,
            <Self as ::encase::private::ShaderType>::METADATA.padding(0usize)
                as ::core::primitive::usize,
        );
        let v = ::encase::private::CreateFrom::create_from(reader);
        ::encase::private::Reader::advance(
            reader,
            <Self as ::encase::private::ShaderType>::METADATA.padding(1usize)
                as ::core::primitive::usize,
        );
        let f = ::encase::private::CreateFrom::create_from(reader);
        ::encase::private::Reader::advance(
            reader,
            <Self as ::encase::private::ShaderType>::METADATA.padding(2usize)
                as ::core::primitive::usize,
        );
        {
            let mut uninit_struct = ::core::mem::MaybeUninit::<Self>::uninit();
            let ptr = ::core::mem::MaybeUninit::as_mut_ptr(&mut uninit_struct);
            let field_ptr = unsafe { &raw mut (*ptr).m };
            unsafe { field_ptr.write(m) };
            let field_ptr = unsafe { &raw mut (*ptr).v };
            unsafe { field_ptr.write(v) };
            let field_ptr = unsafe { &raw mut (*ptr).f };
            unsafe { field_ptr.write(f) };
            unsafe { ::core::mem::MaybeUninit::assume_init(uninit_struct) }
        }
    }
}
impl ::encase::private::ShaderSize for CalcText1
where
    [f32; 16]: ::encase::private::ShaderSize,
    [f32; 16]: ::encase::private::ShaderSize,
    f32: ::encase::private::ShaderSize,
{
}
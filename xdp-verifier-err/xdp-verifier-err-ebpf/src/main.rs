#![no_std]
#![no_main]

use core::mem;

use aya_bpf::{
    bindings::{xdp_action, TC_ACT_PIPE, TC_ACT_SHOT},
    macros::{classifier, xdp},
    programs::{TcContext, XdpContext},
    BpfContext,
};
use aya_log_ebpf::info;
use network_types::{
    l2::ethernet::{EthHdr, ETH_HDR_LEN},
    l3::{
        ip::{Ipv4Hdr, IPV4_HDR_LEN},
        L3Protocol,
    },
    l4::{tcp::TCP_HDR_LEN, udp::UDP_HDR_LEN, L4Protocol},
};

#[classifier(name = "tc_verifier_err")]
pub fn tc_verifier_err(ctx: TcContext) -> i32 {
    match try_verifier_err(&ctx) {
        Ok(_) => TC_ACT_PIPE,
        Err(_) => TC_ACT_SHOT,
    }
}

#[xdp(name = "xdp_verifier_err")]
pub fn xdp_verifier_err(ctx: XdpContext) -> u32 {
    match try_verifier_err(&ctx) {
        Ok(_) => xdp_action::XDP_PASS,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

trait DirectPacketAccess {
    fn data(&self) -> usize;
    fn data_end(&self) -> usize;
}

impl DirectPacketAccess for TcContext {
    fn data(&self) -> usize {
        self.data()
    }

    fn data_end(&self) -> usize {
        self.data_end()
    }
}

impl DirectPacketAccess for XdpContext {
    fn data(&self) -> usize {
        self.data()
    }

    fn data_end(&self) -> usize {
        self.data_end()
    }
}

#[inline(always)]
unsafe fn ptr_at<C, T>(ctx: &C, offset: usize) -> Result<*const T, ()>
where
    C: BpfContext + DirectPacketAccess,
{
    let start = ctx.data();
    let end = ctx.data_end();
    let len = mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as *const T)
}

#[inline(never)]
fn fetch_foo<C>(ctx: &C, offset: usize) -> Result<(), ()>
where
    C: BpfContext + DirectPacketAccess,
{
    if ctx.data() + offset + 8 > ctx.data_end() {
        return Err(());
    }
    let foo = unsafe { *((ctx.data() + offset) as *const u64) };
    info!(ctx, "foo: {}", foo);

    if ctx.data() + offset + 16 > ctx.data_end() {
        return Err(());
    }
    let foo = unsafe { *((ctx.data() + offset + 8) as *const u64) };
    info!(ctx, "foo: {}", foo);

    Ok(())
}

fn try_verifier_err<C>(ctx: &C) -> Result<(), ()>
where
    C: BpfContext + DirectPacketAccess,
{
    let ethhdr: *const EthHdr = unsafe { ptr_at(ctx, 0)? };
    match unsafe { *ethhdr }.protocol()? {
        L3Protocol::Ipv4 => {}
        _ => return Ok(()),
    }

    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(ctx, ETH_HDR_LEN)? };
    let offset = match unsafe { *ipv4hdr }.protocol()? {
        L4Protocol::Tcp => ETH_HDR_LEN + IPV4_HDR_LEN + TCP_HDR_LEN,
        L4Protocol::Udp => ETH_HDR_LEN + IPV4_HDR_LEN + UDP_HDR_LEN,
        _ => return Err(()),
    };

    fetch_foo(ctx, offset)?;

    Ok(())
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}

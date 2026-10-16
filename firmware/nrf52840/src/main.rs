//! Tablet-attached XIAO: ordinary Lattice participation plus a binary USB host link.
#![no_std]
#![no_main]
#![allow(static_mut_refs)]
use core::{
    ffi::c_void,
    fmt::Write,
    future::{poll_fn, Future},
    pin::pin,
    task::{Context, Poll, Waker},
};
use embassy_futures::join::join;
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, Config,
};
use hal::{
    bind_interrupts,
    interrupt::{self, InterruptExt},
    peripherals, usb,
};
use lattice_vertex::{
    kernel::{self, host},
    port::{self, nrf52840::hal},
    sync::{TaskContext, TaskHandle},
    types::StackType_t,
};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => usb::vbus_detect::InterruptHandler;
});
static mut USB: Option<hal::Peri<'static, peripherals::USBD>> = None;
static mut STACK: [StackType_t; 2048] = [0; 2048];
static mut CONTROL: kernel::tasks::StaticTask_t = kernel::tasks::StaticTask_t::new();

struct Serial {
    bytes: [u8; 16],
    len: usize,
}
impl core::fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        let out = self.bytes.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        out.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
struct UsbSession {
    dtr: bool,
    epoch: u32,
    reset_since: Option<u32>,
}
impl UsbSession {
    fn update(&mut self, context: &TaskContext, dtr: bool, rate: u32) {
        if self.dtr && !dtr {
            host::disconnect(context);
            self.epoch = self.epoch.wrapping_add(1);
        }
        self.dtr = dtr;
        if rate != 1200 || dtr {
            self.reset_since = None;
            return;
        }
        let now = kernel::tasks::xTaskGetTickCount();
        let since = *self.reset_since.get_or_insert(now);
        if now.wrapping_sub(since) >= lattice_vertex::milliseconds_to_ticks_checked(100).unwrap() {
            hal::pac::POWER.gpregret().write(|w| w.set_gpregret(0x4e));
            cortex_m::peripheral::SCB::sys_reset();
        }
    }
}
extern "C" fn usb_task(_: *mut c_void) {
    let context = unsafe { TaskContext::assume() };
    interrupt::USBD.set_priority(interrupt::Priority::P6);
    interrupt::CLOCK_POWER.set_priority(interrupt::Priority::P6);
    let driver = usb::Driver::new(
        unsafe { USB.take().unwrap() },
        Irqs,
        usb::vbus_detect::HardwareVbusDetect::new(Irqs),
    );
    let mut serial = Serial {
        bytes: [0; 16],
        len: 0,
    };
    write!(
        serial,
        "{:08X}{:08X}",
        hal::pac::FICR.deviceid(1).read(),
        hal::pac::FICR.deviceid(0).read()
    )
    .unwrap();
    // Development identity, distinct from the earlier diagnostic firmware.
    let mut config = Config::new(0xc0de, 0xcafd);
    config.manufacturer = Some("Lattice");
    config.product = Some("Tablet radio");
    config.serial_number = Some(core::str::from_utf8(&serial.bytes).unwrap());
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    let mut descriptors = [0; 256];
    let mut bos = [0; 64];
    let mut msos = [0; 64];
    let mut control_buffer = [0; 64];
    let mut class_state = State::new();
    let mut builder = Builder::new(
        driver,
        config,
        &mut descriptors,
        &mut bos,
        &mut msos,
        &mut control_buffer,
    );
    let class = CdcAcmClass::new(&mut builder, &mut class_state, 64);
    let (mut sender, mut receiver) = class.split();
    let mut device = builder.build();
    let exchange = async {
        let mut decoder = host::Decoder::new();
        let mut session = UsbSession {
            dtr: false,
            epoch: 0,
            reset_since: None,
        };
        let mut seen_epoch = 0;
        let mut input = [0; 64];
        let mut output = [0; host::MAX_PACKET];
        loop {
            receiver.wait_connection().await;
            let received = {
                let mut read = pin!(receiver.read_packet(&mut input));
                poll_fn(|cx| {
                    session.update(&context, sender.dtr(), sender.line_coding().data_rate());
                    read.as_mut().poll(cx)
                })
                .await
            };
            let count = match received {
                Ok(n) => n,
                Err(_) => {
                    host::disconnect(&context);
                    decoder = host::Decoder::new();
                    continue;
                }
            };
            if session.epoch != seen_epoch {
                seen_epoch = session.epoch;
                decoder = host::Decoder::new();
            }
            if !receiver.dtr() {
                continue;
            }
            for byte in &input[..count] {
                let Some(request) = decoder.push(*byte) else {
                    continue;
                };
                let response = match host::submit(&context, request) {
                    Err(failure) => Some(host::Packet::Error {
                        token: request.token(),
                        failure,
                    }),
                    Ok(()) => {
                        poll_fn(|_| {
                            session.update(
                                &context,
                                sender.dtr(),
                                sender.line_coding().data_rate(),
                            );
                            if session.epoch != seen_epoch || !sender.dtr() {
                                return Poll::Ready(None);
                            }
                            host::receive(&context)
                                .map_or(Poll::Pending, |reply| Poll::Ready(Some(reply)))
                        })
                        .await
                    }
                };
                if let Some(response) = response {
                    let n = response.encode(&mut output);
                    let written = {
                        let mut result = Ok(());
                        // Endpoint packets do not delimit protocol records.
                        for chunk in output[..n]
                            .chunks(64)
                            .chain((n % 64 == 0).then_some(&[][..]))
                        {
                            let mut write = pin!(sender.write_packet(chunk));
                            result = poll_fn(|cx| {
                                session.update(
                                    &context,
                                    receiver.dtr(),
                                    receiver.line_coding().data_rate(),
                                );
                                write.as_mut().poll(cx)
                            })
                            .await;
                            if result.is_err() {
                                break;
                            }
                        }
                        result
                    };
                    if written.is_err() {
                        host::disconnect(&context);
                        decoder = host::Decoder::new();
                        break;
                    }
                }
            }
        }
    };
    let mut work = pin!(join(device.run(), exchange));
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        let _ = work.as_mut().poll(&mut cx);
        lattice_vertex::delay_ms(&context, 1);
    }
}
#[cortex_m_rt::entry]
fn main() -> ! {
    let core = cortex_m::Peripherals::take().unwrap();
    let p = port::nrf52840::initialize(core.SYST);
    let context = unsafe { TaskContext::assume() };
    unsafe {
        USB = Some(p.USBD);
        TaskHandle::spawn_static(&context, b"host-usb", &mut STACK, &mut CONTROL, 1, usb_task)
            .unwrap();
    }
    static APPLIANCE: kernel::Appliance<(), core::convert::Infallible> =
        kernel::Appliance::new(&lattice_vertex::surface::EMPTY);
    APPLIANCE.initialize(&context, ());
    kernel::start(context, p.RADIO, &APPLIANCE)
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    port::portDISABLE_INTERRUPTS();
    // Preserve software recovery even if this bring-up application fails.
    hal::pac::POWER.gpregret().write(|w| w.set_gpregret(0x4e));
    cortex_m::peripheral::SCB::sys_reset()
}

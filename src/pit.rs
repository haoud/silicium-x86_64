use crate::io::Port;

static CHANNEL_0: Port<u8> = unsafe { Port::new(0x40) };
static CHANNEL_1: Port<u8> = unsafe { Port::new(0x41) };
static CHANNEL_2: Port<u8> = unsafe { Port::new(0x42) };
static COMMAND: Port<u8> = unsafe { Port::new(0x43) };

const PIT_TICK_NS: u64 = 1_000_000_000 / 1_193_180;
const PIT_FREQ: u64 = 1_193_180;
const MAX_FREQ: u64 = PIT_FREQ / 2;
const MIN_FREQ: u64 = 1;

/// Represents a Programmable Interval Timer (PIT).
pub struct Pit {
    frequency: u64,
    latch: u64,
}

impl Pit {
    /// Creates a new PIT with the given frequency. This function does not configure the PIT, you
    /// must call `setup` to do that.
    ///
    /// # Panics
    /// Panics if the frequency is lower than 1 Hz or greater than 596590 Hz.
    pub const fn new(freq: u64) -> Self {
        assert!(freq >= MIN_FREQ, "PIT frequency cannot be lower than 1 Hz",);
        assert!(
            freq <= MAX_FREQ,
            "PIT frequency cannot be greater than 596590 Hz",
        );
        Self {
            frequency: freq,
            latch: PIT_FREQ / freq,
        }
    }

    /// Sets the frequency of the PIT and configures it to generate square waves on channel 0.
    /// IRQ will be fired every time the counter reaches 0 on IRQ 0: You must set and handle the IRQ
    /// yourself.
    pub fn setup(&self) {
        let low = (self.latch & 0xFF) as u8;
        let high = ((self.latch >> 8) & 0xFF) as u8;

        // Set channel 0 to mode 3 (square wave generator), binary format
        // and set the frequency divisor
        COMMAND.write(0x36);
        CHANNEL_0.write(low);
        CHANNEL_0.write(high);
    }

    /// Returns the elapsed time since the last IRQ in nanoseconds. In order to do that, it reads the
    /// current value of the counter and calculates the elapsed time since the last IRQ. Since this
    /// function read through the PIT and I/O ports, it is not very fast, and should not be called
    /// often.
    pub fn nano_offset(&self) -> u64 {
        // Read the current value of the counter (channel 0)
        COMMAND.write(0);
        let low = CHANNEL_0.read() as u64;
        let high = CHANNEL_0.read() as u64;
        let counter = (high << 8) | low;

        // Calculate the elapsed time since the last IRQ
        let elapsed = self.latch - counter;
        elapsed * PIT_TICK_NS
    }

    /// Returns the frequency of the PIT, in Hz.
    pub const fn get_frequency(&self) -> u64 {
        self.frequency
    }
}

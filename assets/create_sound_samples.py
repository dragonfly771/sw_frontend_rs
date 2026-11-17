import math

# Samples per period of the DAC waveform.
#
# The DAC sample rate is SAMPLES_COUNT * fundamental, so the first image of the staircase sits
# at (SAMPLES_COUNT - 1) * fundamental. With 20 samples a 180 Hz sink tone put that image at
# 3.4 kHz - right where the ear is most sensitive, which is what made the low tones sound
# whistly and scratchy. 64 samples move it to 11 kHz and above.
SAMPLES_COUNT = 64
# Number of steps of the volume table - independent of the waveform resolution, it maps the
# gain values 31..50 onto amplitude factors.
VOLUME_STEPS = 20
MAX_AMPLITUDE = 4095
HALF_AMPLITUDE = MAX_AMPLITUDE // 2

# Harmonics of the band limited triangle ("wood") of the tone lab:
# fundamental + 0.11 * 3rd + 0.04 * 5th harmonic.
#
# A naive 20 sample triangle contains everything up to the Nyquist frequency and therefore
# aliases audibly - it sounds harder than the "wood" preset of the tone lab. The band limited
# sum below only holds partials up to the 5th harmonic, which 20 samples per period can
# represent without aliasing.
TRIANGLE_HARMONICS = [(1, 1.0), (3, 0.11), (5, 0.04)]

# The tone lab additionally runs a lowpass that tracks the pitch (cutoff = 3.0 x fundamental,
# Q = 0.7). Because the cutoff is a fixed multiple of the fundamental, its effect is the same at
# every pitch and can be baked into the table. Set this to (3.0, 0.7) to reproduce the lab
# exactly; leave it at None for the composition as written in the concept (section 3.6).
TRIANGLE_TRACKING_LPF = (3.0, 0.7)

def lpf_gain(harmonic, cutoff_ratio, q):
    """Magnitude of a 2nd order lowpass at 'harmonic' times the fundamental."""
    r = harmonic / cutoff_ratio
    return 1.0 / math.sqrt((1.0 - r * r) ** 2 + (r / q) ** 2)

def triangular_wave():
    harmonics = TRIANGLE_HARMONICS
    if TRIANGLE_TRACKING_LPF is not None:
        cutoff_ratio, q = TRIANGLE_TRACKING_LPF
        harmonics = [(h, a * lpf_gain(h, cutoff_ratio, q)) for h, a in harmonics]
    count = SAMPLES_COUNT
    dy = 2 * math.pi / count
    raw = []
    y = 0.0
    for _ in range(count):
        y += dy
        value = 0.0
        for harmonic, amplitude in harmonics:
            value += amplitude * math.sin(harmonic * y)
        raw.append(value)
    # normalize to the same peak amplitude as the other waveforms
    peak = max(abs(value) for value in raw)
    return [2047.0 * value / peak for value in raw]

def sawtooth_wave():
    delta = 2 * HALF_AMPLITUDE / SAMPLES_COUNT
    count = SAMPLES_COUNT // 2
    samples = []

    value = 0.0
    for _ in range(count):
        value += delta
        samples.append(value)

    value = float(-HALF_AMPLITUDE - delta)

    for _ in range(count):
        value += delta
        samples.append(value)
    return samples

def rectangular_wave():
    delta = float(HALF_AMPLITUDE)
    count = SAMPLES_COUNT // 2
    samples = []
    for _ in range(count):
        samples.append( + delta)
    for _ in range(count):
        samples.append( - delta)
    return samples
    
def sine_wave():
    factor = 2047.0
    count = SAMPLES_COUNT
    dy = 2 * math.pi / count
    samples = []
    y = 0.0
    for _ in range(count):
        y += dy
        samples.append(factor * math.sin(y))
    return samples

def volume():
    samples = []
    for idx in range(VOLUME_STEPS):
        samples.append(0.9659363**(VOLUME_STEPS - 1 - idx))
    return samples

def get_samples(f, name, exact, size_const="SAMPLES_COUNT"):
    to_store = []

    if exact:
        for sample in f():
            to_store.append(f"{sample:0.5f}")
    else:
        for sample in f():
            to_store.append(f"{sample:0.1f}")

    s = '\n'
    s += "#[allow(unused)]\n"
    s += "#[rustfmt::skip]\n"
    s += f"pub const {name}: [f32; {size_const}] = [\n"
    # wrap the long lines so the generated file stays readable
    lines, line = [], "   "
    for value in to_store:
        if len(line) + len(value) + 2 > 100:
            lines.append(line)
            line = "   "
        line += " " + value + ","
    lines.append(line.rstrip(","))
    s += "\n".join(lines)
    s += "];\n"
    return s

def get_file_content():
    content = "// Created by 'create_sound_samples.py' - do not edit\n\n"
    content += f"pub const SAMPLES_COUNT: usize = {SAMPLES_COUNT};\n"
    content += f"pub const VOLUME_STEPS: usize = {VOLUME_STEPS};\n"
    content += get_samples(triangular_wave, "TRIANGULAR_WAVE", False)
    content += get_samples(sawtooth_wave, "SAWTOOTH_WAVE", False)
    content += get_samples(rectangular_wave, "RECTANGULAR_WAVE", False)
    content += get_samples(sine_wave, "SINE_WAVE", False)
    content += get_samples(volume, "VOLUME_FACTORS", True, "VOLUME_STEPS")
    return content

def write_to_file(file_name, content):
    with open(file_name, 'w') as f:
        f.write(content)
    print(f"  file '{file_name}' created")

print("create_sound_samples.py")
content = get_file_content()
write_to_file('device/larus_frontend_v2/src/utils/samples.rs', content)

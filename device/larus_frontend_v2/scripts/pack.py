#!../../.venv/bin/python

import sys, io, toml, struct

from elftools.elf.elffile import ELFFile
from elftools.elf.relocation import RelocationSection


def stm32_crc(data):
    crc=0xffffffff
    buf = bytearray()
    for b in data:
        buf.insert(0, b)
        if len(buf) == 4:
            for val in buf:
                crc ^= val << 24
                for _ in range(8):
                    crc = crc << 1 if (crc & 0x80000000) == 0 else (crc << 1) ^ 0x104c11db7
            buf = bytearray()
    return crc

# Marker in the firmware (src/utils/version.rs, LARUS_HW_ID) followed by the hardware version
HW_MARKER = b'LARUS-HW-ID:'

# manufacturer.major.minor -> display variant and the part of the file name that marks it.
# Keep in sync with src/utils/version.rs and make_variant.sh.
VARIANTS = {
    (1, 3, 2): ('small', 'larus_frontend_v2_v'),
    (1, 3, 3): ('large', 'larus_frontend_v2_large_v'),
}

def fail(text):
    print(f"\nERROR: {text}")
    print("No image written.")
    sys.exit(1)

def check_hw_marker(app_bin, hw_version_str, image_name):
    """The hardware version in the image header must be the one compiled into the code.
    So an image for the large display can never carry the header (or name) of the small one."""
    pos = app_bin.find(HW_MARKER)
    if pos < 0:
        fail("hardware marker LARUS-HW-ID not found in the app binary")
    if app_bin.find(HW_MARKER, pos + 1) >= 0:
        fail("hardware marker LARUS-HW-ID found more than once")
    hw_code = tuple(app_bin[pos + len(HW_MARKER):pos + len(HW_MARKER) + 4])
    hw_toml = tuple(int(no) for no in hw_version_str.split('.'))
    print(f"\nHardware version in code: {'.'.join(str(b) for b in hw_code)}, in pack.toml: {hw_version_str}")
    if hw_code[:3] != hw_toml[:3]:
        fail("the code was built for another display variant than pack.toml says - use make_variant.sh")
    if hw_code[:3] not in VARIANTS:
        fail("unknown hardware version, add it to VARIANTS")
    variant, prefix = VARIANTS[hw_code[:3]]
    if not image_name.startswith(prefix):
        fail(f"image name '{image_name}' does not fit the variant '{variant}' (must start with '{prefix}')")
    print(f"Display variant:          {variant}")

def get_version(version_str):
        shift_fact = 0
        version = 0
        for no in version_str.split('.'):
            version += int(no) << shift_fact
            shift_fact += 8

        return version

class ReadApp():
    """Read elf file and store all binaries and symbols"""
    def __init__(self, file_name):
        in_stream =  open(file_name, "rb")
        self.elf_file = ELFFile(in_stream, sys.stdout)
        self.file_name = file_name
        with open(file_name, 'rb') as f:
            self.elf_original = f.read()
    
    def get_binary(self, flash_start, flash_end):
        """Return the binary data that lies within the defined range"""
        last_adr = 0
        segment_data = []

        print(f"\nLoading app image from segments in '{self.file_name}'")
        print(f"    {'Address':8}   {'Length':8}")
        for segment in self.elf_file.iter_segments():
            addr = segment['p_paddr']
            # a segment behind the end of the app area (flash bank 1, 512 KB on the
            # STM32H743VG) would silently be left out and the device would not start
            if segment['p_type'] == 'PT_LOAD' and segment['p_filesz'] > 0 \
                    and flash_start <= addr < 0x0810_0000 and addr + segment['p_filesz'] > flash_end + 1:
                fail(f"program too large: segment 0x{addr:08X} + 0x{segment['p_filesz']:X} ends at "
                     f"0x{addr + segment['p_filesz']:08X}, the app area ends at 0x{flash_end + 1:08X} "
                     f"({(addr + segment['p_filesz'] - flash_start) // 1024} KB of {(flash_end + 1 - flash_start) // 1024} KB)")
            if addr >= flash_start and addr < flash_end:
                data = segment.data()
                length = len(data)
        
                print(f"  0x{addr:08X} 0x{length:08X}")
                segment_data.append((addr - flash_start, data))
                last_adr = addr + length

        binary_len = last_adr - flash_start
        print(f"  size {binary_len // 1024} KB of {(flash_end + 1 - flash_start) // 1024} KB, "
              f"free {(flash_end + 1 - last_adr) // 1024} KB")
        binary = bytearray(binary_len)
        for ofs, data in segment_data:
            binary[ofs:ofs + len(data)] = data

        # Be shure, that binary is word aligned
        while len(binary) % 32 != 0: # Go safe to get 32 byte alignment (stm32h7)
            binary += b'\xff'

        self.binary = binary
        return binary
    
    def write_elf_file(self, file_name):
        with open(file_name, 'wb') as f:
            f.write(self.elf_original)

    def get_symbol_address(self, symbol_name):
        """Return the address of the symbol"""
        symtab = self.elf_file.get_section_by_name('.symtab')
        symbol = symtab.get_symbol_by_name(symbol_name)
        return symbol[0].entry['st_value']

class Binary():
    """A class to create binary Larus images"""
    def __init__(self, image):
        """storage_adr: Address where the image is to be stored"""
        self.name = image["name"]
        self.addr_storage = image["addr_storage"]
        self.hw_version = get_version(image["hw_version"])
        self.sw_version = get_version(image["sw_version"])
        self.elf_file_name = image["elf"]

    def read_new_app(self, app):
        """Load the app that is to be executed later"""
        self.new_app = ReadApp(app["elf"])
        self.app_addr_start = app["addr_start"]
        app_addr_max = app["addr_max"]
        self.app_bin = self.new_app.get_binary(self.app_addr_start, app_addr_max)

    def read_copy_app(self, copy):
        """Load the copy routine that loads the future app in the right place."""
        copy_app = ReadApp(copy["elf"])
        self.copy_app_addr_start = copy["addr_start"]
        copy_app_addr_max = copy["addr_max"]
        self.copy_bin = copy_app.get_binary(self.copy_app_addr_start, copy_app_addr_max)
        self.copy_func = copy_app.get_symbol_address("main")

    def create_meta_data(self):
        """Create the meta data needed"""
        data = {
             'Magic Number': 0x1c80_73ab_2085_3579,
             'CRC <place holder>': 0x12345678,
             'Meta Data Version': 1,
             'Storage Address': self.addr_storage,
             'Hardware Version': self.hw_version,
             'Software Version': self.sw_version,
             'Copy Function': self.copy_func,
             'New App': self.addr_storage + 0x1000 + len(self.copy_bin),
             'New App Len': len(self.app_bin),
             'New App Dest': self.app_addr_start
        }
        print('\nCreating Meta Data:')
        for key, value in data.items():
            print(f"  {key:21}0x{value:08X}")


        self.meta_data = struct.pack ('<QLLLLLLLLL', *data.values())
        while len(self.meta_data) < (self.copy_app_addr_start - self.addr_storage): # Fill til copy 
            self.meta_data += b'\x00'

    def write_files(self):
        """Save binary to disk"""
        self.create_meta_data()
        binary = bytearray(self.meta_data + self.copy_bin + self.app_bin)

        print("\nCalculate CRC 'Storage Address' -> <end>")
        crc_data = stm32_crc(binary[12:]) # Start at storag_adr -> end
        binary[8:12] = struct.pack("<L", crc_data)
        print(f"  CRC inserted         0x{crc_data:08X}")

        print(f"\nTotal size of image file: {round(len(binary) / 1024)}k")
        print(f"Writing binary to file '{self.name}'")
        with open(self.name, "wb") as bin_file:
            bin_file.write(binary)

        # the bare program for STM32CubeProgrammer / ST-Link at 0x08000000
        raw_name = self.name[:-4] + f"_app_0x{self.app_addr_start:08X}.bin"
        print(f"Writing program for the ST-Link to '{raw_name}' (address 0x{self.app_addr_start:08X})")
        with open(raw_name, "wb") as raw_file:
            raw_file.write(self.app_bin)

        print(f"Write elf file to '{self.elf_file_name}'")
        self.new_app.write_elf_file(self.elf_file_name)

print("Larus App Image Packer")
with open("pack.toml", "r") as f:
    spec = toml.load(f)
    image = Binary(spec["image"])
    image.read_new_app(spec["app"])
    check_hw_marker(image.app_bin, spec["image"]["hw_version"], spec["image"]["name"])
    image.read_copy_app(spec["copy"])
    image.write_files()

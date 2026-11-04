"""Run the MAME PROM update block independently. No firmware is copied to git."""
from pathlib import Path
import subprocess
import sys
import tempfile

source = Path(sys.argv[1]).read_text()
block = source.split("\t\t\taddress = (address & 0x1e)", 1)[1].split("\t\t\tcycles++;", 1)[0]
block = "address = (address & 0x1e)" + block
block = block.replace("floppy && floppy->wpt_r()", "wp")
wrapper = r'''
#include <cstdint>
#include <fstream>
#include <iostream>
int main(int argc, char** argv) {
 if (argc != 2) return 2;
 uint8_t m_rom_p6[256];
 std::ifstream in(argv[1],std::ios::binary);
 if (!in.read(reinterpret_cast<char*>(m_rom_p6),256) || in.peek()!=EOF) return 3;
 uint8_t address=0,data_reg=0;
 uint32_t seed=1;
 for(int n=0;n<1024;n++) {
  seed=seed*1664525u+1013904223u;
  bool q6=seed&1,q7=seed&2,pulse=seed&4,wp=seed&8;
  uint8_t last_6502_write=seed>>16;
  address=(address&0xe3)|(q6?4:0)|(q7?8:0)|(pulse?0:16);
  uint8_t opcode=m_rom_p6[address];
  BLOCK
  std::cout << q6 << ',' << q7 << ',' << pulse << ',' << wp << ',' << int(last_6502_write) << ',' << int(address) << ',' << int(data_reg) << '\n';
 }
}
'''.replace("BLOCK", block)
with tempfile.TemporaryDirectory(prefix="disk-ii-reference-") as folder:
    cpp = Path(folder) / "reference.cpp"
    exe = Path(folder) / "reference"
    cpp.write_text(wrapper)
    subprocess.run(["c++", "-std=c++17", str(cpp), "-o", str(exe)], check=True)
    subprocess.run([str(exe), sys.argv[2]], check=True)

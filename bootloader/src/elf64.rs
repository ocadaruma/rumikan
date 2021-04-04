#[repr(C)]
#[derive(Debug, Eq, PartialEq)]
pub struct FileHeader {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Eq, PartialEq)]
pub struct ProgramHeader {
    pub p_type: SegmentType,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[repr(u32)]
#[derive(Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum SegmentType {
    Null = 0x0,
    Load = 0x1,
    Dynamic,
    Interpreter,
    Note,
    Shlib,
    Tls,
    Loos = 0x60000000,
    His = 0x6fffffff,
    Loproc = 0x70000000,
    Hiproc = 0x7fffffff,
}

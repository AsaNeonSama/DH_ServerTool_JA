export const STD_INPUT_HANDLE: number = -10;
export const STD_OUTPUT_HANDLE: number = -11;
export const ENABLE_WINDOW_INPUT: number = 0x0008;
export const ENABLE_MOUSE_INPUT: number = 0x0010;
export const ENABLE_QUICK_EDIT_MODE: number = 0x0040;
export const ENABLE_EXTENDED_FLAGS: number = 0x0080;
export const ENABLE_VIRTUAL_TERMINAL_INPUT: number = 0x0200;
export const ENABLE_VIRTUAL_TERMINAL_PROCESSING: number = 0x0004;
export const CP_UTF8: number = 65001;
export const CF_UNICODETEXT: number = 13;
export const GMEM_MOVEABLE: number = 0x0002;
export const TH32CS_SNAPPROCESS: number = 0x00000002;
export const PROCESS_QUERY_LIMITED_INFORMATION: number = 0x1000;
export const PROCESS_SET_QUOTA: number = 0x0100;
export const PROCESS_TERMINATE: number = 0x0001;
export const STILL_ACTIVE: number = 259;

// Win64 JOBOBJECT_EXTENDED_LIMIT_INFORMATION (size = 144 bytes / 0x90, LimitFlags at +0x10)
export const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: number = 9;
export const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: number = 0x00002000;
export const JOBOBJECT_EXTENDED_LIMIT_INFO_SIZE: number = 144;
export const JOBOBJECT_LIMIT_FLAGS_OFFSET: number = 16;
export const AF_INET: number = 2;
export const AF_INET6: number = 23;
export const UDP_TABLE_OWNER_PID: number = 1;
export const TCP_TABLE_OWNER_PID_LISTENER: number = 3;
export const ERROR_INSUFFICIENT_BUFFER: number = 122;
export const INVALID_HANDLE_VALUE: bigint = -1n;

// Win64 PROCESSENTRY32W struct size = 568 bytes (0x238)
export const PROCESSENTRY32W_SIZE: number = 568;
export const PE32W_PID_OFFSET: number = 0x08;
export const PE32W_PPID_OFFSET: number = 0x20;
export const PE32W_EXEFILE_OFFSET: number = 0x2c;
export const PE32W_EXEFILE_MAX_BYTES: number = 260 * 2;

// MIB_UDPROW_OWNER_PID / MIB_UDP6ROW_OWNER_PID / MIB_TCPROW_OWNER_PID layout constants
export const UDP_V4_ROW_SIZE: number = 12;
export const UDP_V4_PORT_OFFSET: number = 4;
export const UDP_V4_PID_OFFSET: number = 8;
export const UDP_V6_ROW_SIZE: number = 28;
export const UDP_V6_PORT_OFFSET: number = 20;
export const UDP_V6_PID_OFFSET: number = 24;
export const TCP_V4_ROW_SIZE: number = 24;
export const TCP_V4_PORT_OFFSET: number = 8;
export const IP_TABLE_MIN_BUFFER_SIZE: number = 4096;
export const IP_TABLE_EXTRA_BUFFER_SIZE: number = 2048;

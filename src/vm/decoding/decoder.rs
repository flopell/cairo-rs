use crate::{
    types::instruction::{
        ApUpdate, FpUpdate, Instruction, Op1Addr, Opcode, PcUpdate, Register, Res,
    },
    vm::errors::vm_errors::VirtualMachineError,
};
use felt::Felt;

//  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
// 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0

/// Decodes an instruction. The encoding is little endian, so flags go from bit 63 to 48.
pub fn decode_instruction(
    encoded_instr: i64,
    mut imm: Option<&Felt>,
) -> Result<Instruction, VirtualMachineError> {
    const DST_REG_MASK: i64 = 0x0001;
    const DST_REG_OFF: i64 = 0;
    const OP0_REG_MASK: i64 = 0x0002;
    const OP0_REG_OFF: i64 = 1;
    const OP1_SRC_MASK: i64 = 0x001C;
    const OP1_SRC_OFF: i64 = 2;
    const RES_LOGIC_MASK: i64 = 0x0060;
    const RES_LOGIC_OFF: i64 = 5;
    const PC_UPDATE_MASK: i64 = 0x0380;
    const PC_UPDATE_OFF: i64 = 7;
    const AP_UPDATE_MASK: i64 = 0x0C00;
    const AP_UPDATE_OFF: i64 = 10;
    const OPCODE_MASK: i64 = 0x7000;
    const OPCODE_OFF: i64 = 12;

    // Flags start on the 48th bit.
    const FLAGS_OFFSET: i64 = 48;
    const OFF0_OFF: i64 = 0;
    const OFF1_OFF: i64 = 16;
    const OFF2_OFF: i64 = 32;
    const OFFX_MASK: i64 = 0xFFFF;

    // Grab offsets and convert them from little endian format.
    let off0 = decode_offset(encoded_instr >> OFF0_OFF & OFFX_MASK);
    let off1 = decode_offset(encoded_instr >> OFF1_OFF & OFFX_MASK);
    let off2 = decode_offset(encoded_instr >> OFF2_OFF & OFFX_MASK);

    // Grab flags
    let flags = encoded_instr >> FLAGS_OFFSET;
    // Grab individual flags
    let dst_reg_num = (flags & DST_REG_MASK) >> DST_REG_OFF;
    let op0_reg_num = (flags & OP0_REG_MASK) >> OP0_REG_OFF;
    let op1_src_num = (flags & OP1_SRC_MASK) >> OP1_SRC_OFF;
    let res_logic_num = (flags & RES_LOGIC_MASK) >> RES_LOGIC_OFF;
    let pc_update_num = (flags & PC_UPDATE_MASK) >> PC_UPDATE_OFF;
    let ap_update_num = (flags & AP_UPDATE_MASK) >> AP_UPDATE_OFF;
    let opcode_num = (flags & OPCODE_MASK) >> OPCODE_OFF;

    // Match each flag to its corresponding enum value
    let dst_register = if dst_reg_num == 1 {
        Register::FP
    } else {
        Register::AP
    };

    let op0_register = if op0_reg_num == 1 {
        Register::FP
    } else {
        Register::AP
    };

    let op1_addr = match op1_src_num {
        0 => Op1Addr::Op0,
        1 => Op1Addr::Imm,
        2 => Op1Addr::FP,
        4 => Op1Addr::AP,
        _ => return Err(VirtualMachineError::InvalidOp1Reg(op1_src_num)),
    };

    if op1_addr == Op1Addr::Imm {
        if imm.is_none() {
            return Err(VirtualMachineError::NoImm);
        }
    } else {
        imm = None
    }

    let pc_update = match pc_update_num {
        0 => PcUpdate::Regular,
        1 => PcUpdate::Jump,
        2 => PcUpdate::JumpRel,
        4 => PcUpdate::Jnz,
        _ => return Err(VirtualMachineError::InvalidPcUpdate(pc_update_num)),
    };

    let res = match res_logic_num {
        0 if matches!(pc_update, PcUpdate::Jnz) => Res::Unconstrained,
        0 => Res::Op1,
        1 => Res::Add,
        2 => Res::Mul,
        _ => return Err(VirtualMachineError::InvalidRes(res_logic_num)),
    };

    let opcode = match opcode_num {
        0 => Opcode::NOp,
        1 => Opcode::Call,
        2 => Opcode::Ret,
        4 => Opcode::AssertEq,
        _ => return Err(VirtualMachineError::InvalidOpcode(opcode_num)),
    };

    let ap_update = match ap_update_num {
        0 if matches!(opcode, Opcode::Call) => ApUpdate::Add2,
        0 => ApUpdate::Regular,
        1 => ApUpdate::Add,
        2 => ApUpdate::Add1,
        _ => return Err(VirtualMachineError::InvalidApUpdate(ap_update_num)),
    };

    let fp_update = match opcode {
        Opcode::Call => FpUpdate::APPlus2,
        Opcode::Ret => FpUpdate::Dst,
        _ => FpUpdate::Regular,
    };

    Ok(Instruction {
        off0,
        off1,
        off2,
        imm: imm.cloned(),
        dst_register,
        op0_register,
        op1_addr,
        res,
        pc_update,
        ap_update,
        fp_update,
        opcode,
    })
}

fn decode_offset(offset: i64) -> isize {
    let vectorized_offset: [u8; 8] = offset.to_le_bytes();
    let offset_16b_encoded = u16::from_le_bytes([vectorized_offset[0], vectorized_offset[1]]);
    let complement_const = 0x8000u16;
    let (offset_16b, _) = offset_16b_encoded.overflowing_sub(complement_const);
    isize::from(offset_16b as i16)
}

#[cfg(test)]
mod decoder_test {
    use super::*;

    #[test]
    fn invalid_op1_reg() {
        let error = decode_instruction(0x294F800080008000, None);
        assert_eq!(error, Err(VirtualMachineError::InvalidOp1Reg(3)));
        assert_eq!(
            error.unwrap_err().to_string(),
            "Invalid op1_register value: 3"
        )
    }

    #[test]
    fn invalid_pc_update() {
        let error = decode_instruction(0x29A8800080008000, None);
        assert_eq!(error, Err(VirtualMachineError::InvalidPcUpdate(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid pc_update value: 3")
    }

    #[test]
    fn invalid_res_logic() {
        let error = decode_instruction(0x2968800080008000, None);
        assert_eq!(error, Err(VirtualMachineError::InvalidRes(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid res value: 3")
    }

    #[test]
    fn invalid_opcode() {
        let error = decode_instruction(0x3948800080008000, None);
        assert_eq!(error, Err(VirtualMachineError::InvalidOpcode(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid opcode value: 3")
    }

    #[test]
    fn invalid_ap_update() {
        let error = decode_instruction(0x2D48800080008000, None);
        assert_eq!(error, Err(VirtualMachineError::InvalidApUpdate(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid ap_update value: 3")
    }

    #[test]
    fn decode_no_immediate_given() {
        assert_eq!(
            decode_instruction(0x14A7800080008000, None),
            Err(VirtualMachineError::NoImm)
        );
    }

    #[test]
    fn decode_flags_call_add_jmp_add_imm_fp_fp() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |    CALL|      ADD|     JUMP|      ADD|    IMM|     FP|     FP
        //  0  0  0  1      0  1   0  0  1      0  1 0  0  1       1       1
        //  0001 0100 1010 0111 = 0x14A7; offx = 0
        let inst = decode_instruction(0x14A7800080008000, Some(&Felt::new(7))).unwrap();
        assert!(matches!(inst.dst_register, Register::FP));
        assert!(matches!(inst.op0_register, Register::FP));
        assert!(matches!(inst.op1_addr, Op1Addr::Imm));
        assert!(matches!(inst.res, Res::Add));
        assert!(matches!(inst.pc_update, PcUpdate::Jump));
        assert!(matches!(inst.ap_update, ApUpdate::Add));
        assert!(matches!(inst.opcode, Opcode::Call));
        assert!(matches!(inst.fp_update, FpUpdate::APPlus2));
    }

    #[test]
    fn decode_flags_ret_add1_jmp_rel_mul_fp_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |     RET|     ADD1| JUMP_REL|      MUL|     FP|     AP|     AP
        //  0  0  1  0      1  0   0  1  0      1  0 0  1  0       0       0
        //  0010 1001 0100 1000 = 0x2948; offx = 0
        let inst = decode_instruction(0x2948800080008000, None).unwrap();
        assert!(matches!(inst.dst_register, Register::AP));
        assert!(matches!(inst.op0_register, Register::AP));
        assert!(matches!(inst.op1_addr, Op1Addr::FP));
        assert!(matches!(inst.res, Res::Mul));
        assert!(matches!(inst.pc_update, PcUpdate::JumpRel));
        assert!(matches!(inst.ap_update, ApUpdate::Add1));
        assert!(matches!(inst.opcode, Opcode::Ret));
        assert!(matches!(inst.fp_update, FpUpdate::Dst));
    }

    #[test]
    fn decode_flags_assrt_add_jnz_mul_ap_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |ASSRT_EQ|      ADD|      JNZ|      MUL|     AP|     AP|     AP
        //  0  1  0  0      1  0   1  0  0      1  0 1  0  0       0       0
        //  0100 1010 0101 0000 = 0x4A50; offx = 0
        let inst = decode_instruction(0x4A50800080008000, None).unwrap();
        assert!(matches!(inst.dst_register, Register::AP));
        assert!(matches!(inst.op0_register, Register::AP));
        assert!(matches!(inst.op1_addr, Op1Addr::AP));
        assert!(matches!(inst.res, Res::Mul));
        assert!(matches!(inst.pc_update, PcUpdate::Jnz));
        assert!(matches!(inst.ap_update, ApUpdate::Add1));
        assert!(matches!(inst.opcode, Opcode::AssertEq));
        assert!(matches!(inst.fp_update, FpUpdate::Regular));
    }

    #[test]
    fn decode_flags_assrt_add2_jnz_uncon_op0_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |ASSRT_EQ|     ADD2|      JNZ|UNCONSTRD|    OP0|     AP|     AP
        //  0  1  0  0      0  0   1  0  0      0  0 0  0  0       0       0
        //  0100 0010 0000 0000 = 0x4200; offx = 0
        let inst = decode_instruction(0x4200800080008000, None).unwrap();
        assert!(matches!(inst.dst_register, Register::AP));
        assert!(matches!(inst.op0_register, Register::AP));
        assert!(matches!(inst.op1_addr, Op1Addr::Op0));
        assert!(matches!(inst.res, Res::Unconstrained));
        assert!(matches!(inst.pc_update, PcUpdate::Jnz));
        assert!(matches!(inst.ap_update, ApUpdate::Regular));
        assert!(matches!(inst.opcode, Opcode::AssertEq));
        assert!(matches!(inst.fp_update, FpUpdate::Regular));
    }

    #[test]
    fn decode_flags_nop_regu_regu_op1_op0_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |     NOP|  REGULAR|  REGULAR|      OP1|    OP0|     AP|     AP
        //  0  0  0  0      0  0   0  0  0      0  0 0  0  0       0       0
        //  0000 0000 0000 0000 = 0x0000; offx = 0
        let inst = decode_instruction(0x0000800080008000, None).unwrap();
        assert!(matches!(inst.dst_register, Register::AP));
        assert!(matches!(inst.op0_register, Register::AP));
        assert!(matches!(inst.op1_addr, Op1Addr::Op0));
        assert!(matches!(inst.res, Res::Op1));
        assert!(matches!(inst.pc_update, PcUpdate::Regular));
        assert!(matches!(inst.ap_update, ApUpdate::Regular));
        assert!(matches!(inst.opcode, Opcode::NOp));
        assert!(matches!(inst.fp_update, FpUpdate::Regular));
    }

    #[test]
    fn decode_offset_negative() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |     NOP|  REGULAR|  REGULAR|      OP1|    OP0|     AP|     AP
        //  0  0  0  0      0  0   0  0  0      0  0 0  0  0       0       0
        //  0000 0000 0000 0000 = 0x0000; offx = 0
        let inst = decode_instruction(0x0000800180007FFF, None).unwrap();
        assert_eq!(inst.off0, -1);
        assert_eq!(inst.off1, 0);
        assert_eq!(inst.off2, 1);
    }
}

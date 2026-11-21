//go:build amd64 && !purego

#include "textflag.h"

// func hasPCLMULQDQ() bool
TEXT ·hasPCLMULQDQ(SB), NOSPLIT, $0-1
	MOVL $1, AX
	XORL CX, CX
	CPUID
	SHRL $1, CX
	ANDL $1, CX
	MOVB CX, ret+0(FP)
	RET

// func clmulVec54(a, b, lo, hi *uint64)
// Computes 54 carryless 64x64->128-bit polynomial multiplications over GF(2) in ONE call,
// 2x-unrolled with 128-bit MOVOU + dual PCLMULQDQ ($0x00 / $0x11) for high ILP!
TEXT ·clmulVec54(SB), NOSPLIT, $0-32
	MOVQ a+0(FP), SI
	MOVQ b+8(FP), DI
	MOVQ lo+16(FP), R8
	MOVQ hi+24(FP), R9
	MOVQ $27, CX
loop27:
	MOVOU (SI), X0
	MOVOU (DI), X1
	MOVO X0, X2
	PCLMULQDQ $0x00, X1, X0
	PCLMULQDQ $0x11, X1, X2
	MOVQ X0, 0(R8)
	PEXTRQ $1, X0, 0(R9)
	MOVQ X2, 8(R8)
	PEXTRQ $1, X2, 8(R9)
	ADDQ $16, SI
	ADDQ $16, DI
	ADDQ $16, R8
	ADDQ $16, R9
	DECQ CX
	JNZ loop27
	RET

// func ringSwitchApplyASM(table *[96][16]E, z *E, out *E)
// Evaluates the 384-bit GF(2)-linear ring-switching map using 4-bit nibble tables (48 B per E) and SSE2 PXOR.
TEXT ·ringSwitchApplyASM(SB), NOSPLIT, $0-24
	MOVQ table+0(FP), DI
	MOVQ z+8(FP), SI
	MOVQ out+16(FP), R8
	PXOR X0, X0
	PXOR X1, X1
	PXOR X2, X2
	MOVQ $48, CX
rs_loop:
	MOVBLZX (SI), AX
	TESTQ AX, AX
	JZ rs_next
	// Low nibble: offset = nib * 48 = (nib*3) << 4
	MOVQ AX, DX
	ANDQ $0x0F, DX
	LEAQ (DX)(DX*2), DX
	SHLQ $4, DX
	LEAQ (DI)(DX*1), BX
	PXOR 0(BX), X0
	PXOR 16(BX), X1
	PXOR 32(BX), X2
	// High nibble: offset = 768 + hi_nib * 48
	SHRQ $4, AX
	LEAQ (AX)(AX*2), AX
	SHLQ $4, AX
	LEAQ 768(DI)(AX*1), BX
	PXOR 0(BX), X0
	PXOR 16(BX), X1
	PXOR 32(BX), X2
rs_next:
	ADDQ $1, SI
	ADDQ $1536, DI
	DECQ CX
	JNZ rs_loop
	MOVOU X0, 0(R8)
	MOVOU X1, 16(R8)
	MOVOU X2, 32(R8)
	RET

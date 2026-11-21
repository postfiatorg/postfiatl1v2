//go:build !amd64 || purego

package main

const usePCLMUL = false

func (e E) mulPCLMUL(rhs E) E {
	return e.mulPortable(rhs, getKTables())
}

func (m *RingSwitchLinearMap) applyFast(z E) E {
	return m.applyPortable(z)
}

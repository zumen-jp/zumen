package zumen

import (
	"encoding/binary"
	"errors"
	"fmt"
	"math"
	"unicode/utf8"
)

const (
	sceneHeader = 16
	sceneEntry  = 8
)

// Scene は、シーンの blob（決定 0137）の読み手。
//
// 見出し（16 バイト）と区画の表（区画ごとに 8 バイト）を読み、区画を名前で引く。値はリトルエンディアン。知らない区画は
// 読み飛ばし、blob にない区画は空の列にする。大版が違えば読まない。
type Scene struct {
	// Major は、大版。
	Major uint16

	// Minor は、小版。
	Minor uint16

	// Chunks は、塊の数。
	Chunks uint32

	// Sections は、blob にある区画の数。
	Sections uint32

	columns map[string][]byte
}

// ReadScene は、blob を読む。区画は blob を写さずに指す。
func ReadScene(blob []byte) (*Scene, error) {
	if len(blob) < sceneHeader || string(blob[:4]) != "ZMSC" {
		return nil, errors.New("シーンの blob でない")
	}
	scene := &Scene{
		Major:    binary.LittleEndian.Uint16(blob[4:]),
		Minor:    binary.LittleEndian.Uint16(blob[6:]),
		Chunks:   binary.LittleEndian.Uint32(blob[8:]),
		Sections: binary.LittleEndian.Uint32(blob[12:]),
		columns:  map[string][]byte{},
	}
	if scene.Major != SceneMajor {
		return nil, fmt.Errorf("シーンの大版が %d（この包みは %d）", scene.Major, SceneMajor)
	}
	if uint64(sceneHeader)+uint64(sceneEntry)*uint64(scene.Sections) > uint64(len(blob)) {
		return nil, errors.New("区画の表が blob に収まらない")
	}
	for index, section := range SceneSections {
		if uint32(index) >= scene.Sections {
			scene.columns[section.Name] = nil
			continue
		}
		at := sceneHeader + sceneEntry*index
		start := uint64(binary.LittleEndian.Uint32(blob[at:]))
		length := uint64(binary.LittleEndian.Uint32(blob[at+4:]))
		row := uint64(section.Type.Size() * section.Width)
		if start+length > uint64(len(blob)) || length%row != 0 {
			return nil, fmt.Errorf("区画 %s の始まりか長さが合わない", section.Name)
		}
		scene.columns[section.Name] = blob[start : start+length]
	}
	return scene, nil
}

func sceneSection(name string) (SceneSection, error) {
	for _, section := range SceneSections {
		if section.Name == name {
			return section, nil
		}
	}
	return SceneSection{}, fmt.Errorf("知らない区画: %s", name)
}

func (s *Scene) column(name string, columnType ColumnType) ([]byte, error) {
	section, err := sceneSection(name)
	if err != nil {
		return nil, err
	}
	if section.Type != columnType {
		return nil, fmt.Errorf("区画 %s の値の型が違う", name)
	}
	return s.columns[name], nil
}

// Buffer は、区画のバイト列（写さない）。値はリトルエンディアン。
func (s *Scene) Buffer(name string) ([]byte, error) {
	section, err := sceneSection(name)
	if err != nil {
		return nil, err
	}
	return s.column(name, section.Type)
}

// Rows は、区画の行の数。
func (s *Scene) Rows(name string) (int, error) {
	section, err := sceneSection(name)
	if err != nil {
		return 0, err
	}
	return len(s.columns[name]) / (section.Type.Size() * section.Width), nil
}

// U8 は、u8 の区画の値（写さない）。
func (s *Scene) U8(name string) ([]byte, error) {
	return s.column(name, ColumnU8)
}

// U32 は、u32 の区画の値。
func (s *Scene) U32(name string) ([]uint32, error) {
	data, err := s.column(name, ColumnU32)
	if err != nil {
		return nil, err
	}
	out := make([]uint32, len(data)/4)
	for i := range out {
		out[i] = binary.LittleEndian.Uint32(data[4*i:])
	}
	return out, nil
}

// F32 は、f32 の区画の値。
func (s *Scene) F32(name string) ([]float32, error) {
	data, err := s.column(name, ColumnF32)
	if err != nil {
		return nil, err
	}
	out := make([]float32, len(data)/4)
	for i := range out {
		out[i] = math.Float32frombits(binary.LittleEndian.Uint32(data[4*i:]))
	}
	return out, nil
}

// F64 は、f64 の区画の値。
func (s *Scene) F64(name string) ([]float64, error) {
	data, err := s.column(name, ColumnF64)
	if err != nil {
		return nil, err
	}
	out := make([]float64, len(data)/8)
	for i := range out {
		out[i] = math.Float64frombits(binary.LittleEndian.Uint64(data[8*i:]))
	}
	return out, nil
}

// Text は、strings の区画から、文字列を引く（UTF-8）。
func (s *Scene) Text(start, length uint32) (string, error) {
	strings, err := s.column("strings", ColumnU8)
	if err != nil {
		return "", err
	}
	end := uint64(start) + uint64(length)
	if end > uint64(len(strings)) {
		return "", errors.New("strings の区画の外を指す")
	}
	text := strings[start:end]
	if !utf8.Valid(text) {
		return "", errors.New("strings の区画の文字列が UTF-8 でない")
	}
	return string(text), nil
}

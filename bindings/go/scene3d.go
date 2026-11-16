package zumen

import (
	"encoding/binary"
	"errors"
	"fmt"
	"math"
	"unicode/utf8"
)

const (
	scene3DHeader = 16
	scene3DEntry  = 8
)

// Scene3D は、シーンの blob（決定 0213）のリーダー。
//
// ヘッダー（16 バイト）とセクションの表（セクションごとに 8 バイト）を読み、セクションを名前で引く。値はリトルエンディアン。知らないセクションは
// 読み飛ばし、基本の 20 セクションを必要とする。メジャーバージョンが違えば読まない。
type Scene3D struct {
	// Major は、メジャーバージョン。
	Major uint16

	// Minor は、マイナーバージョン。
	Minor uint16

	// Chunks は、塊の数。
	Chunks uint32

	// Sections は、blob にあるセクションの数。
	Sections uint32

	columns map[string][]byte
}

// ReadScene3D は、blob を読む。セクションは blob を写さずに指す。
func ReadScene3D(blob []byte) (*Scene3D, error) {
	if len(blob) > 536870912 {
		return nil, errors.New("3D シーンのバイト数の上限")
	}
	if len(blob) < scene3DHeader || string(blob[:4]) != "ZMS3" {
		return nil, errors.New("シーンの blob でない")
	}
	scene := &Scene3D{
		Major:    binary.LittleEndian.Uint16(blob[4:]),
		Minor:    binary.LittleEndian.Uint16(blob[6:]),
		Chunks:   binary.LittleEndian.Uint32(blob[8:]),
		Sections: binary.LittleEndian.Uint32(blob[12:]),
		columns:  map[string][]byte{},
	}
	if scene.Major != Scene3DMajor {
		return nil, fmt.Errorf("シーンのメジャーバージョンが %d（このラッパーは %d）", scene.Major, Scene3DMajor)
	}
	if uint64(scene3DHeader)+uint64(scene3DEntry)*uint64(scene.Sections) > uint64(len(blob)) {
		return nil, errors.New("セクションの表が blob に収まらない")
	}
	if scene.Sections < 20 {
		return nil, errors.New("3D シーンの基本列がない")
	}
	previous := uint64(scene3DHeader) + uint64(scene3DEntry)*uint64(scene.Sections)
	for i := uint32(0); i < scene.Sections; i++ {
		at := scene3DHeader + scene3DEntry*int(i)
		start, length := uint64(binary.LittleEndian.Uint32(blob[at:])), uint64(binary.LittleEndian.Uint32(blob[at+4:]))
		if start%8 != 0 || start < previous || start+length > uint64(len(blob)) {
			return nil, errors.New("3D シーンのセクションの範囲")
		}
		previous = start + length
	}
	for index, section := range Scene3DSections {
		if uint32(index) >= scene.Sections {
			scene.columns[section.Name] = nil
			continue
		}
		at := scene3DHeader + scene3DEntry*index
		start := uint64(binary.LittleEndian.Uint32(blob[at:]))
		length := uint64(binary.LittleEndian.Uint32(blob[at+4:]))
		row := uint64(section.Type.Size() * section.Width)
		if start+length > uint64(len(blob)) || length%row != 0 {
			return nil, fmt.Errorf("セクション %s の始まりか長さが合わない", section.Name)
		}
		scene.columns[section.Name] = blob[start : start+length]
	}
	return scene, nil
}

func scene3DSection(name string) (SceneSection, error) {
	for _, section := range Scene3DSections {
		if section.Name == name {
			return section, nil
		}
	}
	return SceneSection{}, fmt.Errorf("知らないセクション: %s", name)
}

func (s *Scene3D) column(name string, columnType ColumnType) ([]byte, error) {
	section, err := scene3DSection(name)
	if err != nil {
		return nil, err
	}
	if section.Type != columnType {
		return nil, fmt.Errorf("セクション %s の値の型が違う", name)
	}
	return s.columns[name], nil
}

// Buffer は、セクションのバイト列（写さない）。値はリトルエンディアン。
func (s *Scene3D) Buffer(name string) ([]byte, error) {
	section, err := scene3DSection(name)
	if err != nil {
		return nil, err
	}
	return s.column(name, section.Type)
}

// Rows は、セクションの行の数。
func (s *Scene3D) Rows(name string) (int, error) {
	section, err := scene3DSection(name)
	if err != nil {
		return 0, err
	}
	return len(s.columns[name]) / (section.Type.Size() * section.Width), nil
}

// U8 は、u8 のセクションの値（写さない）。
func (s *Scene3D) U8(name string) ([]byte, error) {
	return s.column(name, ColumnU8)
}

// U16 は、u16 のセクションの値。
func (s *Scene3D) U16(name string) ([]uint16, error) {
	data, err := s.column(name, ColumnU16)
	if err != nil {
		return nil, err
	}
	out := make([]uint16, len(data)/2)
	for i := range out {
		out[i] = binary.LittleEndian.Uint16(data[2*i:])
	}
	return out, nil
}

// U32 は、u32 のセクションの値。
func (s *Scene3D) U32(name string) ([]uint32, error) {
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

// F32 は、f32 のセクションの値。
func (s *Scene3D) F32(name string) ([]float32, error) {
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

// F64 は、f64 のセクションの値。
func (s *Scene3D) F64(name string) ([]float64, error) {
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

// Text は、strings のセクションから、文字列を引く（UTF-8）。
func (s *Scene3D) Text(start, length uint32) (string, error) {
	strings, err := s.column("strings", ColumnU8)
	if err != nil {
		return "", err
	}
	end := uint64(start) + uint64(length)
	if end > uint64(len(strings)) {
		return "", errors.New("strings のセクションの外を指す")
	}
	text := strings[start:end]
	if !utf8.Valid(text) {
		return "", errors.New("strings のセクションの文字列が UTF-8 でない")
	}
	return string(text), nil
}

func (s *Scene3D) reference(name string, row int, component int) (uint32, error) {
	section, err := scene3DSection(name)
	if err != nil {
		return 0, err
	}
	count, err := s.Rows(name)
	if err != nil {
		return 0, err
	}
	if row < 0 || row >= count || component < 0 || component >= section.Width {
		return 0, errors.New("3D シーンの行の範囲")
	}
	return binary.LittleEndian.Uint32(s.columns[name][(row*section.Width+component)*4:]), nil
}
func (s *Scene3D) instanceReference(name, target string, row int) (uint32, error) {
	value, err := s.reference(name, row, 0)
	if err != nil {
		return 0, err
	}
	count, err := s.Rows(target)
	if err != nil {
		return 0, err
	}
	if uint64(value) >= uint64(count) {
		return 0, errors.New("3D シーンの参照")
	}
	return value, nil
}

// ElementID は、元の建物の要素 ID。
func (s *Scene3D) ElementID(row int) (string, error) {
	start, err := s.reference("element_ids", row, 0)
	if err != nil {
		return "", err
	}
	length, err := s.reference("element_ids", row, 1)
	if err != nil {
		return "", err
	}
	return s.Text(start, length)
}

// InstanceMesh は、インスタンスが指すメッシュの行。
func (s *Scene3D) InstanceMesh(row int) (uint32, error) {
	return s.instanceReference("instance_meshes", "mesh_vertices", row)
}

// InstanceElement は、インスタンスが指す要素の行。
func (s *Scene3D) InstanceElement(row int) (uint32, error) {
	return s.instanceReference("instance_elements", "element_ids", row)
}

// InstanceColor は、インスタンスが指す色の行。
func (s *Scene3D) InstanceColor(row int) (uint32, error) {
	return s.instanceReference("instance_colors", "color_rgba", row)
}

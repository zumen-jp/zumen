VERSION 1.0 CLASS
BEGIN
  MultiUse = -1
END
Attribute VB_Name = "ZumenScene3D"
Option Explicit

Private mData() As Byte
Private mStarts() As Long
Private mLengths() As Long
Private mMinor As Long
Private mChunks As Double
Private mSections As Double
Private mReady As Boolean

Public Sub Read(ByRef data() As Byte)
    Dim index As Long, start As Double, length As Double, tableEnd As Double, size As Long
    mReady = False
    Erase mData
    ReDim mStarts(0 To ZM_SCENE3D_SECTION_COUNT - 1)
    ReDim mLengths(0 To ZM_SCENE3D_SECTION_COUNT - 1)
    size = ZmByteCount(data)
    If size < 16 Then Err.Raise 5, "Zumen", "Invalid scene header"
    If size > 536870912 Then Err.Raise 5, "Zumen", "3D scene byte limit"
    mData = data
    If ZmReadU32(mData, 0) <> &H33534D5A Then Err.Raise 5, "Zumen", "Invalid scene magic"
    If ReadU16(4) <> ZM_SCENE3D_MAJOR Then Err.Raise 5, "Zumen", "Scene major mismatch"
    mMinor = ReadU16(6)
    mChunks = ZmReadU32(mData, 8)
    mSections = ZmReadU32(mData, 12)
    tableEnd = 16# + 8# * mSections
    If tableEnd > size Then Err.Raise 5, "Zumen", "Scene table outside buffer"
    If mSections < ZM_SCENE3D_SECTION_COUNT Then Err.Raise 5, "Zumen", "Missing 3D scene sections"
    Dim previous As Double
    previous = tableEnd
    For index = 0 To CLng(mSections) - 1
        start = ZmReadU32(mData, 16 + 8 * index)
        length = ZmReadU32(mData, 20 + 8 * index)
        If start / 8# <> Fix(start / 8#) Or start < previous Or start + length > size Then Err.Raise 5, "Zumen", "Invalid 3D scene range"
        previous = start + length
    Next index
    For index = 0 To ZM_SCENE3D_SECTION_COUNT - 1
        If index < mSections Then
            start = ZmReadU32(mData, 16 + 8 * index)
            length = ZmReadU32(mData, 20 + 8 * index)
            If start > size Or length > size - start Then Err.Raise 5, "Zumen", "Scene section outside buffer"
            If length > 0 And (start < tableEnd Or start / 8# <> Fix(start / 8#)) Then Err.Raise 5, "Zumen", "Invalid scene alignment"
            If length / RowSize(index) <> Fix(length / RowSize(index)) Then Err.Raise 5, "Zumen", "Invalid scene row size"
            mStarts(index) = CLng(start)
            mLengths(index) = CLng(length)
        End If
    Next index
    mReady = True
End Sub

Public Property Get Minor() As Long
    CheckReady
    Minor = mMinor
End Property

Public Property Get Chunks() As Double
    CheckReady
    Chunks = mChunks
End Property

Public Property Get Sections() As Double
    CheckReady
    Sections = mSections
End Property

Public Function Rows(ByVal sectionName As String) As Long
    Dim index As Long
    CheckReady
    index = ZmScene3DIndex(sectionName)
    Rows = mLengths(index) / RowSize(index)
End Function

Public Function Value(ByVal sectionName As String, ByVal row As Long, Optional ByVal component As Long = 0) As Double
    Dim index As Long, position As Long, size As Long
    CheckReady
    index = ZmScene3DIndex(sectionName)
    If row < 0 Or row >= Rows(sectionName) Then Err.Raise 9, "Zumen", "Scene row outside section"
    If component < 0 Or component >= ZmScene3DSectionWidth(index) Then Err.Raise 9, "Zumen", "Scene component outside row"
    size = ItemSize(index)
    position = mStarts(index) + row * RowSize(index) + component * size
    Select Case ZmScene3DSectionType(index)
        Case "u8": Value = mData(LBound(mData) + position)
        Case "u32": Value = ZmReadU32(mData, position)
        Case Else: Value = ZmReadFloat(mData, position, size)
    End Select
End Function

Public Function Text(ByVal start As Long, ByVal length As Long) As String
    Dim index As Long, part() As Byte, offset As Long
    CheckReady
    index = ZmScene3DIndex("strings")
    If start < 0 Or length < 0 Or CDbl(start) + length > mLengths(index) Then Err.Raise 9, "Zumen", "Scene string outside section"
    If length = 0 Then Exit Function
    ReDim part(0 To length - 1)
    For offset = 0 To length - 1
        part(offset) = mData(LBound(mData) + mStarts(index) + start + offset)
    Next offset
    Text = ZmUtf8Decode(part)
End Function

Private Sub CheckReady()
    If Not mReady Then Err.Raise 5, "Zumen", "Scene is not loaded"
End Sub

Private Function ReadU16(ByVal offset As Long) As Long
    ReadU16 = CLng(mData(LBound(mData) + offset)) + 256& * mData(LBound(mData) + offset + 1)
End Function

Private Function ItemSize(ByVal index As Long) As Long
    Select Case ZmScene3DSectionType(index)
        Case "u8": ItemSize = 1
        Case "u32", "f32": ItemSize = 4
        Case "f64": ItemSize = 8
    End Select
End Function

Private Function RowSize(ByVal index As Long) As Long
    RowSize = ItemSize(index) * ZmScene3DSectionWidth(index)
End Function

Private Function ZmScene3DIndex(ByVal name As String) As Long
    Dim index As Long
    For index = 0 To ZM_SCENE3D_SECTION_COUNT - 1
        If ZmScene3DSectionName(index) = name Then
            ZmScene3DIndex = index
            Exit Function
        End If
    Next index
    Err.Raise 5, "Zumen", "Unknown 3D scene section"
End Function

Public Function ElementId(ByVal row As Long) As String
    Dim start As Double, length As Double
    start = Value("element_ids", row, 0)
    length = Value("element_ids", row, 1)
    If start > 536870912# Or length > 536870912# Then Err.Raise 9, "Zumen", "3D scene string range"
    ElementId = Text(CLng(start), CLng(length))
End Function

Private Function InstanceReference(ByVal name As String, ByVal target As String, ByVal row As Long) As Long
    Dim index As Double
    index = Value(name, row)
    If index >= Rows(target) Then Err.Raise 9, "Zumen", "3D scene reference range"
    InstanceReference = CLng(index)
End Function

Public Function InstanceMesh(ByVal row As Long) As Long
    InstanceMesh = InstanceReference("instance_meshes", "mesh_vertices", row)
End Function

Public Function InstanceElement(ByVal row As Long) As Long
    InstanceElement = InstanceReference("instance_elements", "element_ids", row)
End Function

Public Function InstanceColor(ByVal row As Long) As Long
    InstanceColor = InstanceReference("instance_colors", "color_rgba", row)
End Function

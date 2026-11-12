VERSION 1.0 CLASS
BEGIN
  MultiUse = -1
END
Attribute VB_Name = "ZumenScene"
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
    ReDim mStarts(0 To ZM_SCENE_SECTION_COUNT - 1)
    ReDim mLengths(0 To ZM_SCENE_SECTION_COUNT - 1)
    size = ZmByteCount(data)
    If size < 16 Then Err.Raise 5, "Zumen", "Invalid scene header"
    mData = data
    If ZmReadU32(mData, 0) <> &H43534D5A Then Err.Raise 5, "Zumen", "Invalid scene magic"
    If ReadU16(4) <> ZM_SCENE_MAJOR Then Err.Raise 5, "Zumen", "Scene major mismatch"
    mMinor = ReadU16(6)
    mChunks = ZmReadU32(mData, 8)
    mSections = ZmReadU32(mData, 12)
    tableEnd = 16# + 8# * mSections
    If tableEnd > size Then Err.Raise 5, "Zumen", "Scene table outside buffer"
    For index = 0 To ZM_SCENE_SECTION_COUNT - 1
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
    index = ZmSceneIndex(sectionName)
    Rows = mLengths(index) / RowSize(index)
End Function

Public Function Value(ByVal sectionName As String, ByVal row As Long, Optional ByVal component As Long = 0) As Double
    Dim index As Long, position As Long, size As Long
    CheckReady
    index = ZmSceneIndex(sectionName)
    If row < 0 Or row >= Rows(sectionName) Then Err.Raise 9, "Zumen", "Scene row outside section"
    If component < 0 Or component >= ZmSceneWidth(index) Then Err.Raise 9, "Zumen", "Scene component outside row"
    size = ItemSize(index)
    position = mStarts(index) + row * RowSize(index) + component * size
    Select Case ZmSceneType(index)
        Case "u8": Value = mData(LBound(mData) + position)
        Case "u32": Value = ZmReadU32(mData, position)
        Case Else: Value = ZmReadFloat(mData, position, size)
    End Select
End Function

Public Function Text(ByVal start As Long, ByVal length As Long) As String
    Dim index As Long, part() As Byte, offset As Long
    CheckReady
    index = ZmSceneIndex("strings")
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
    Select Case ZmSceneType(index)
        Case "u8": ItemSize = 1
        Case "u32", "f32": ItemSize = 4
        Case "f64": ItemSize = 8
    End Select
End Function

Private Function RowSize(ByVal index As Long) As Long
    RowSize = ItemSize(index) * ZmSceneWidth(index)
End Function

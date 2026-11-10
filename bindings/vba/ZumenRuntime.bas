Attribute VB_Name = "ZumenRuntime"
Option Explicit

Private Declare PtrSafe Sub CopyMemory Lib "kernel32" Alias "RtlMoveMemory" (ByRef target As Any, ByRef source As Any, ByVal length As LongPtr)
Private Declare PtrSafe Function WideCharToMultiByte Lib "kernel32" (ByVal codePage As Long, ByVal flags As Long, ByVal wide As LongPtr, ByVal wideLength As Long, ByVal utf8 As LongPtr, ByVal utf8Length As Long, ByVal defaultChar As LongPtr, ByVal usedDefault As LongPtr) As Long
Private Declare PtrSafe Function MultiByteToWideChar Lib "kernel32" (ByVal codePage As Long, ByVal flags As Long, ByVal utf8 As LongPtr, ByVal utf8Length As Long, ByVal wide As LongPtr, ByVal wideLength As Long) As Long

Public Function ZmByteCount(ByRef data() As Byte) As Long
    Dim first As Long, last As Long
    On Error GoTo EmptyArray
    first = LBound(data)
    last = UBound(data)
    On Error GoTo 0
    If CDbl(last) - first + 1# > 2147483647# Then Err.Raise 6, "Zumen", "Byte array too large"
    ZmByteCount = last - first + 1
    Exit Function
EmptyArray:
    ZmByteCount = 0
End Function

Public Function ZmBytePointer(ByRef data() As Byte) As LongPtr
    If ZmByteCount(data) <> 0 Then ZmBytePointer = VarPtr(data(LBound(data)))
End Function

Public Function ZmUtf8Encode(ByVal text As String) As Byte()
    Dim data() As Byte, size As Long
    If Len(text) = 0 Then
        ZmUtf8Encode = data
        Exit Function
    End If
    size = WideCharToMultiByte(65001, &H80, StrPtr(text), Len(text), 0, 0, 0, 0)
    If size = 0 Then Err.Raise 5, "Zumen", "Invalid UTF-16"
    ReDim data(0 To size - 1)
    If WideCharToMultiByte(65001, &H80, StrPtr(text), Len(text), VarPtr(data(0)), size, 0, 0) <> size Then Err.Raise 5, "Zumen", "UTF-8 encoding failed"
    ZmUtf8Encode = data
End Function

Public Function ZmUtf8Decode(ByRef data() As Byte) As String
    Dim size As Long, count As Long, text As String
    count = ZmByteCount(data)
    If count = 0 Then Exit Function
    size = MultiByteToWideChar(65001, &H8, ZmBytePointer(data), count, 0, 0)
    If size = 0 Then Err.Raise 5, "Zumen", "Invalid UTF-8"
    text = String$(size, vbNullChar)
    If MultiByteToWideChar(65001, &H8, ZmBytePointer(data), count, StrPtr(text), size) <> size Then Err.Raise 5, "Zumen", "UTF-8 decoding failed"
    ZmUtf8Decode = text
End Function

Public Sub ZmCopyBuffer(ByRef buffer As ZmBuf, ByRef data() As Byte)
    Erase data
    If buffer.length < 0 Or buffer.length > 2147483647# Then Err.Raise 6, "Zumen", "Native buffer too large"
    If buffer.length = 0 Then Exit Sub
    If buffer.ptr = 0 Then Err.Raise 5, "Zumen", "Null native buffer"
    ReDim data(0 To CLng(buffer.length) - 1)
    CopyMemory data(0), ByVal buffer.ptr, buffer.length
End Sub

Public Function ZmSigned32(ByVal value As Double) As Long
    If value < 0 Or value > 4294967295# Or value <> Fix(value) Then Err.Raise 5, "Zumen", "Invalid u32"
    If value > 2147483647# Then value = value - 4294967296#
    ZmSigned32 = CLng(value)
End Function

Public Function ZmReadU32(ByRef data() As Byte, ByVal offset As Long) As Double
    Dim value As Long
    If offset < 0 Or CDbl(offset) + 4# > ZmByteCount(data) Then Err.Raise 9, "Zumen", "u32 outside buffer"
    CopyMemory value, data(LBound(data) + offset), 4
    ZmReadU32 = CDbl(value)
    If value < 0 Then ZmReadU32 = ZmReadU32 + 4294967296#
End Function

Public Function ZmReadFloat(ByRef data() As Byte, ByVal offset As Long, ByVal size As Long) As Double
    Dim small As Single, large As Double
    If size <> 4 And size <> 8 Then Err.Raise 5, "Zumen", "Invalid float size"
    If offset < 0 Or CDbl(offset) + size > ZmByteCount(data) Then Err.Raise 9, "Zumen", "Float outside buffer"
    If size = 4 Then
        CopyMemory small, data(LBound(data) + offset), 4
        ZmReadFloat = small
    Else
        CopyMemory large, data(LBound(data) + offset), 8
        ZmReadFloat = large
    End If
End Function

Public Function ZmTsvRows(ByVal text As String) As Variant
    Dim lines As Variant, fields As Variant, result() As Variant, row As Long, col As Long, count As Long, width As Long
    If Len(text) = 0 Then
        ZmTsvRows = Empty
        Exit Function
    End If
    lines = Split(text, vbLf)
    count = UBound(lines) + 1
    If lines(count - 1) = "" Then count = count - 1
    fields = Split(lines(0), vbTab)
    width = UBound(fields) + 1
    ReDim result(1 To count, 1 To width)
    For row = 0 To count - 1
        fields = Split(lines(row), vbTab)
        If UBound(fields) + 1 <> width Then Err.Raise 5, "Zumen", "Invalid TSV row"
        For col = 0 To width - 1
            result(row + 1, col + 1) = ZmTsvUnescape(CStr(fields(col)))
        Next col
    Next row
    ZmTsvRows = result
End Function

Private Function ZmTsvUnescape(ByVal value As String) As String
    Dim index As Long, part As String, result As String
    index = 1
    Do While index <= Len(value)
        part = Mid$(value, index, 1)
        If part = "\" Then
            index = index + 1
            Select Case Mid$(value, index, 1)
                Case "n": part = vbLf
                Case "t": part = vbTab
                Case "r": part = vbCr
                Case "\": part = "\"
                Case Else: Err.Raise 5, "Zumen", "Invalid TSV escape"
            End Select
        End If
        result = result & part
        index = index + 1
    Loop
    ZmTsvUnescape = result
End Function

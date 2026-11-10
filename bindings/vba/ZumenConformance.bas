Attribute VB_Name = "ZumenConformance"
Option Explicit

Private Declare PtrSafe Function BCryptOpenAlgorithmProvider Lib "bcrypt" (ByRef algorithm As LongPtr, ByVal name As LongPtr, ByVal implementation As LongPtr, ByVal flags As Long) As Long
Private Declare PtrSafe Function BCryptCloseAlgorithmProvider Lib "bcrypt" (ByVal algorithm As LongPtr, ByVal flags As Long) As Long
Private Declare PtrSafe Function BCryptHash Lib "bcrypt" (ByVal algorithm As LongPtr, ByVal secret As LongPtr, ByVal secretLength As Long, ByVal data As LongPtr, ByVal dataLength As Long, ByVal digest As LongPtr, ByVal digestLength As Long) As Long

Public Sub ZumenConformance(ByVal fixturePath As String)
    Dim session As ZumenSession, file As Integer, line As String, fields As Variant
    Dim name() As Byte, data() As Byte, response() As Byte, actual As Long, hashes As Long, steps As Long, scenes As Long
    Dim location As String, number As Long, description As String, opened As Boolean
    On Error GoTo Failed
    ZumenSelfTest
    Set session = New ZumenSession
    file = FreeFile
    Open fixturePath For Input As #file
    opened = True
    Do While Not EOF(file)
        Line Input #file, line
        fields = Split(line, vbTab)
        If UBound(fields) <> 6 Then Err.Raise 5, "Zumen", "Invalid fixture row"
        location = fields(0) & ":" & fields(1)
        data = FromHex(CStr(fields(5)))
        Erase response
        Select Case fields(2)
            Case "session": actual = session.OpenBytes(data)
            Case "push"
                name = FromHex(CStr(fields(4)))
                actual = session.PushBytes(name, data)
            Case "call": actual = session.CallBytes(data, response)
            Case "take": actual = session.Take(CDbl(fields(4)), response)
            Case Else: Err.Raise 5, "Zumen", "Unknown fixture command"
        End Select
        Require actual = CLng(fields(3)), "Status mismatch " & location
        If fields(6) <> "" Then
            Require Sha256(response) = fields(6), "Hash mismatch " & location
            hashes = hashes + 1
        Else
            Require ZmByteCount(response) = 0, "Unexpected response " & location
        End If
        If fields(2) = "take" And ZmByteCount(response) >= 16 Then
            If ZmReadU32(response, 0) = &H43534D5A Then
                CheckScene response
                scenes = scenes + 1
            End If
        End If
        steps = steps + 1
    Loop
    Close #file
    opened = False
    session.CloseSession
    Debug.Print "VBA conformance: " & steps & " steps, " & hashes & " hashes, " & scenes & " scenes"
    Exit Sub
Failed:
    number = Err.Number
    description = Err.Description
    If opened Then Close #file
    If Not session Is Nothing Then session.CloseSession
    Err.Raise number, "ZumenConformance", description
End Sub

Private Sub Require(ByVal condition As Boolean, ByVal message As String)
    If Not condition Then Err.Raise 5, "ZumenConformance", message
End Sub

Private Function FromHex(ByVal value As String) As Byte()
    Dim data() As Byte, index As Long
    If Len(value) Mod 2 <> 0 Then Err.Raise 5, "Zumen", "Invalid hex length"
    If Len(value) > 0 Then
        ReDim data(0 To Len(value) / 2 - 1)
        For index = 0 To UBound(data)
            data(index) = CByte("&H" & Mid$(value, 2 * index + 1, 2))
        Next index
    End If
    FromHex = data
End Function

Private Function Sha256(ByRef data() As Byte) As String
    Dim algorithm As LongPtr, name As String, digest(0 To 31) As Byte, result As String, index As Long, status As Long
    name = "SHA256"
    If BCryptOpenAlgorithmProvider(algorithm, StrPtr(name), 0, 0) <> 0 Then Err.Raise 5, "Zumen", "SHA-256 provider failed"
    status = BCryptHash(algorithm, 0, 0, ZmBytePointer(data), ZmByteCount(data), VarPtr(digest(0)), 32)
    BCryptCloseAlgorithmProvider algorithm, 0
    If status <> 0 Then Err.Raise 5, "Zumen", "SHA-256 failed"
    For index = 0 To 31
        result = result & Right$("0" & LCase$(Hex$(digest(index))), 2)
    Next index
    Sha256 = result
End Function

Private Sub CheckScene(ByRef data() As Byte)
    Dim scene As ZumenScene, index As Long, name As String, value As Double
    Set scene = New ZumenScene
    scene.Read data
    Require scene.Chunks = scene.Rows("chunk_origins"), "Scene chunk count mismatch"
    For index = 0 To ZM_SCENE_SECTION_COUNT - 1
        name = ZmSceneName(index)
        If scene.Rows(name) > 0 Then
            value = scene.Value(name, 0)
            value = scene.Value(name, scene.Rows(name) - 1, ZmSceneWidth(index) - 1)
        End If
    Next index
    If scene.Rows("line_xy") > 0 Then Require scene.Value("line_xy", 0, 0) = scene.Value("line_xy", 0, 0), "Scene NaN"
End Sub

Public Sub ZumenSelfTest()
    Dim data() As Byte, received() As Byte, buffer As ZmBuf, text As String, value As Variant, session As ZumenSession
    Require LenB(buffer) = 2 * LenB(buffer.ptr), "Buffer layout mismatch"
    data = ZmUtf8Encode("abc")
    Require Sha256(data) = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "SHA-256 mismatch"
    text = ChrW$(&H56F3) & ChrW$(&H9762) & vbNullChar & ChrW$(&HD83D) & ChrW$(&HDE00)
    data = ZmUtf8Encode(text)
    Require ZmUtf8Decode(data) = text, "UTF-8 round trip mismatch"
    Require EncodeRejected(ChrW$(&HD800)), "Invalid UTF-16 accepted"
    data = FromHex("ff")
    Require DecodeRejected(data), "Invalid UTF-8 accepted"
    Erase data
    Require Sha256(data) = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "Empty SHA-256 mismatch"
    Require ZmBytePointer(data) = 0 And ZmByteCount(data) = 0, "Empty array mismatch"
    Require ZmUtf8Decode(data) = "", "Empty string mismatch"
    value = ZmTsvRows("a" & vbTab & "b" & vbLf & "x\ty" & vbTab & "z\\n" & vbLf)
    Require value(2, 1) = "x" & vbTab & "y", "TSV tab mismatch"
    Require value(2, 2) = "z\n", "TSV escape mismatch"
    Require ZmSigned32(4294967295#) = -1, "u32 conversion mismatch"
    CheckSyntheticScene
    Set session = New ZumenSession
    Require session.OpenSession() = ZM_OK, "Default session failed"
    Require session.PushText("empty.txt", "") = ZM_OK, "Empty push failed"
    Require session.CallText("{", text) = ZM_FAILED And Len(text) > 0, "Failure response missing"
    session.CloseSession
    session.CloseSession
    session.Cancel
    Require session.CallBytes(data, received) = ZM_INVALID, "Closed session mismatch"
End Sub

Private Function EncodeRejected(ByVal text As String) As Boolean
    Dim data() As Byte
    On Error GoTo Rejected
    data = ZmUtf8Encode(text)
    Exit Function
Rejected:
    EncodeRejected = True
End Function

Private Function DecodeRejected(ByRef data() As Byte) As Boolean
    Dim text As String
    On Error GoTo Rejected
    text = ZmUtf8Decode(data)
    Exit Function
Rejected:
    DecodeRejected = True
End Function

Private Function SceneRejected(ByRef data() As Byte) As Boolean
    Dim scene As ZumenScene
    On Error GoTo Rejected
    Set scene = New ZumenScene
    scene.Read data
    Exit Function
Rejected:
    SceneRejected = True
End Function

Private Sub CheckSyntheticScene()
    Dim scene As ZumenScene, data() As Byte
    Set scene = New ZumenScene
    data = FromHex("5a4d5343010000000000000000000000")
    scene.Read data
    Require scene.Rows("line_xy") = 0, "Missing section mismatch"
    data(4) = 2
    Require SceneRejected(data), "Unknown major accepted"
    data = FromHex("5a4d53430100030000000000010000001800000010000000000000000000f03f0000000000000040")
    scene.Read data
    Require scene.Minor = 3 And scene.Rows("chunk_origins") = 1, "Scene header mismatch"
    Require scene.Value("chunk_origins", 0, 0) = 1# And scene.Value("chunk_origins", 0, 1) = 2#, "Scene f64 mismatch"
    data(20) = 15
    Require SceneRejected(data), "Partial scene row accepted"
    data(20) = 16
    data(16) = 255
    Require SceneRejected(data), "Invalid scene range accepted"
    data = FromHex("5a4d5343010003000000000001000000")
    Require SceneRejected(data), "Truncated scene table accepted"
End Sub

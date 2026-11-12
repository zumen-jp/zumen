VERSION 1.0 CLASS
BEGIN
  MultiUse = -1
END
Attribute VB_Name = "ZumenSession"
Option Explicit

Private mHandle As LongPtr

Public Function OpenBytes(ByRef options() As Byte) As Long
    CloseSession
    If zm_abi_version() <> ZM_ABI_VERSION Then Err.Raise 5, "Zumen", "ABI version mismatch"
    OpenBytes = zm_session_new(ZmBytePointer(options), ZmByteCount(options), mHandle)
End Function

Public Function OpenSession(Optional ByVal options As String = "") As Long
    Dim data() As Byte
    data = ZmUtf8Encode(options)
    OpenSession = OpenBytes(data)
End Function

Public Function PushBytes(ByRef name() As Byte, ByRef data() As Byte) As Long
    PushBytes = zm_push(mHandle, ZmBytePointer(name), ZmByteCount(name), ZmBytePointer(data), ZmByteCount(data))
End Function

Public Function Push(ByVal name As String, ByRef data() As Byte) As Long
    Dim encoded() As Byte
    encoded = ZmUtf8Encode(name)
    Push = PushBytes(encoded, data)
End Function

Public Function PushText(ByVal name As String, ByVal text As String) As Long
    Dim data() As Byte
    data = ZmUtf8Encode(text)
    PushText = Push(name, data)
End Function

Public Function CallBytes(ByRef request() As Byte, ByRef response() As Byte) As Long
    Dim buffer As ZmBuf, number As Long, description As String
    On Error GoTo Failed
    CallBytes = zm_call(mHandle, ZmBytePointer(request), ZmByteCount(request), buffer)
    ZmCopyBuffer buffer, response
    zm_free buffer
    Exit Function
Failed:
    number = Err.Number
    description = Err.Description
    On Error Resume Next
    zm_free buffer
    On Error GoTo 0
    Err.Raise number, "Zumen", description
End Function

Public Function CallText(ByVal request As String, ByRef response As String) As Long
    Dim data() As Byte, received() As Byte
    response = ""
    data = ZmUtf8Encode(request)
    CallText = CallBytes(data, received)
    response = ZmUtf8Decode(received)
End Function

Public Function Take(ByVal blob As Double, ByRef data() As Byte) As Long
    Dim buffer As ZmBuf, number As Long, description As String
    On Error GoTo Failed
    Take = zm_take(mHandle, ZmSigned32(blob), buffer)
    ZmCopyBuffer buffer, data
    zm_free buffer
    Exit Function
Failed:
    number = Err.Number
    description = Err.Description
    On Error Resume Next
    zm_free buffer
    On Error GoTo 0
    Err.Raise number, "Zumen", description
End Function

Public Sub Cancel()
    zm_cancel mHandle
End Sub

Public Sub CloseSession()
    If mHandle <> 0 Then
        zm_session_close mHandle
        mHandle = 0
    End If
End Sub

Private Sub Class_Terminate()
    CloseSession
End Sub

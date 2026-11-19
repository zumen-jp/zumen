program zumen_summary;

{$APPTYPE CONSOLE}

uses
  System.SysUtils, System.IOUtils, System.JSON;

const
  ZumenDll = 'zumen_abi1.dll';
  InputName: UTF8String = 'drawing.dxf';

type
  TZmBuffer = record
    Data: PAnsiChar;
    Len: NativeUInt;
  end;

function zm_session_new(Options: Pointer; Len: NativeUInt;
  out Session: Pointer): Int32; cdecl; external ZumenDll;
function zm_push(Session: Pointer; Name: PAnsiChar; NameLen: NativeUInt;
  Data: Pointer; DataLen: NativeUInt): Int32; cdecl; external ZumenDll;
function zm_call(Session: Pointer; Request: PAnsiChar; Len: NativeUInt;
  var Response: TZmBuffer): Int32; cdecl; external ZumenDll;
procedure zm_free(var Buffer: TZmBuffer); cdecl; external ZumenDll;
procedure zm_session_close(Session: Pointer); cdecl; external ZumenDll;

function ReadResult(Session: Pointer; const Request: UTF8String;
  const FieldPath: string): string;
var
  Buffer: TZmBuffer;
  Reply: UTF8String;
  Status: Int32;
  Root, Field: TJSONValue;
begin
  Buffer.Data := nil;
  Buffer.Len := 0;
  Status := zm_call(Session, PAnsiChar(Request), Length(Request), Buffer);
  try
    if Buffer.Len > High(Integer) then
      raise Exception.Create('Response too large');
    SetString(Reply, Buffer.Data, Integer(Buffer.Len));
  finally
    zm_free(Buffer);
  end;
  if Status <> 0 then
    raise Exception.CreateFmt('zm_call returned %d: %s', [Status, string(Reply)]);

  Root := TJSONObject.ParseJSONValue(string(Reply));
  try
    if Root = nil then
      raise Exception.Create('Invalid JSON response');
    Field := Root.FindValue(FieldPath);
    if Field = nil then
      raise Exception.Create('Missing response field: ' + FieldPath);
    Result := Field.Value;
  finally
    Root.Free;
  end;
end;

procedure Summarize(const FileName: string);
var
  Data: TBytes;
  Session: Pointer;
  FormatName, EntityCount: string;
begin
  Data := TFile.ReadAllBytes(FileName);
  if zm_session_new(nil, 0, Session) <> 0 then
    raise Exception.Create('zm_session_new failed');
  try
    if zm_push(Session, PAnsiChar(InputName), Length(InputName), Pointer(Data),
      Length(Data)) <> 0 then
      raise Exception.Create('zm_push failed');
    FormatName := ReadResult(Session, '{"v":1,"op":"open","input":"drawing.dxf"}',
      'result.format');
    EntityCount := ReadResult(Session, '{"v":1,"op":"entities","doc":"drawing.dxf","limit":1}',
      'result.total');
    Writeln('format ', FormatName);
    Writeln('entities ', EntityCount);
  finally
    zm_session_close(Session);
  end;
end;

begin
  try
    if ParamCount <> 1 then
      raise Exception.Create('Usage: zumen_summary <drawing.dxf>');
    Summarize(ParamStr(1));
  except
    on E: Exception do
    begin
      Writeln(ErrOutput, E.Message);
      ExitCode := 1;
    end;
  end;
end.

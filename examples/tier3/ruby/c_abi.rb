# zumen を、Ruby の Fiddle（標準の FFI）から C ABI で呼ぶサンプル（#404、決定 0006・0011）。
#
# 使い方: ruby examples/tier3/ruby/c_abi.rb <libzumen_abi1 のパス> <図面のファイル>
#
# 図面を開き、形式と図形の数を書く。関数の形は bindings/c/include/zumen.h にある。

require "fiddle"
require "json"

library, drawing = ARGV
abort "使い方: ruby c_abi.rb <libzumen_abi1 のパス> <図面のファイル>" unless library && drawing

lib = Fiddle.dlopen(library)
fn = ->(name, args, ret) { Fiddle::Function.new(lib[name], args, ret) }
size = Fiddle::TYPE_SIZE_T
ptr = Fiddle::TYPE_VOIDP
int = Fiddle::TYPE_INT
session_new = fn.("zm_session_new", [ptr, size, ptr], int)
push = fn.("zm_push", [ptr, ptr, size, ptr, size], int)
call = fn.("zm_call", [ptr, ptr, size, ptr], int)
free = fn.("zm_free", [ptr], Fiddle::TYPE_VOID)
close = fn.("zm_session_close", [ptr], Fiddle::TYPE_VOID)

out = Fiddle::Pointer.malloc(Fiddle::SIZEOF_VOIDP, Fiddle::RUBY_FREE)
raise "zm_session_new が失敗した" unless session_new.(nil, 0, out).zero?
session = out.ptr

# 要求を送り、応答の JSON を読む。応答のバッファー（zm_buf: ptr と len）は zm_free で返す
request = lambda do |value|
  bytes = JSON.generate(value)
  buf = Fiddle::Pointer.malloc(Fiddle::SIZEOF_VOIDP + Fiddle::SIZEOF_SIZE_T, Fiddle::RUBY_FREE)
  status = call.(session, bytes, bytes.bytesize, buf)
  data = buf.ptr.null? ? nil : buf.ptr.to_s(buf[Fiddle::SIZEOF_VOIDP, Fiddle::SIZEOF_SIZE_T].unpack1("J"))
  free.(buf)
  raise "#{value["op"]} が状態 #{status} を返した: #{data}" unless status.zero?
  JSON.parse(data)
end

name = File.basename(drawing)
data = File.binread(drawing)
raise "zm_push が失敗した" unless push.(session, name, name.bytesize, data, data.bytesize).zero?
opened = request.({ "v" => 1, "op" => "open", "input" => name })
entities = request.({ "v" => 1, "op" => "entities", "doc" => name })
puts "format #{opened["result"]["format"]}"
puts "entities #{entities["result"]["total"]}"
close.(session)

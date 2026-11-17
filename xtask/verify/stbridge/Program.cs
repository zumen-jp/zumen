// 公開サンプルの ST-Bridge 2.0.2 を STBDotNet（MIT）の v202 のクラスで読み、節点・階・部材・断面・鋼材の規格の
// 属性を JSON に書き出す。照らすのは check.py。
//
//   dotnet run --project xtask/verify/stbridge -c Release -- <入力の .stb> <出力の .json>
//
// STBDotNet は XmlSerializer で読む。属性は、要素の名前（XmlElement の名前）と、XmlAttribute・XmlEnum の名前で書く。
// 値を持たない省略可能な属性（*Specified が false）と、既定値（DefaultValue）のままの属性は書かない。
using System.Collections;
using System.ComponentModel;
using System.Globalization;
using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Xml.Serialization;
using STBDotNet.v202;

if (args.Length != 2)
{
    Console.Error.WriteLine("使い方: DumpStb <入力の .stb> <出力の .json>");
    return 2;
}

Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
ST_BRIDGE model;
using (var stream = File.OpenRead(args[0]))
{
    model = (ST_BRIDGE)new XmlSerializer(typeof(ST_BRIDGE)).Deserialize(stream)!;
}

var root = new JsonObject
{
    ["implementation"] = new JsonObject
    {
        ["name"] = "STBDotNet",
        ["version"] = typeof(ST_BRIDGE).Assembly.GetName().Version?.ToString(3),
    },
    ["version"] = model.version,
    ["nodes"] = Elements(model.StbModel.StbNodes),
    ["stories"] = Elements(model.StbModel.StbStories),
    ["members"] = Lists(model.StbModel.StbMembers),
    ["sections"] = Lists(model.StbModel.StbSections),
    ["steels"] = Lists(model.StbModel.StbSections.StbSecSteel),
};
var options = new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping };
File.WriteAllText(args[1], root.ToJsonString(options) + "\n", new UTF8Encoding(false));
return 0;

// 配列の属性を持つ親（StbMembers・StbSections・StbSecSteel）の子を、要素の名前ごとの一覧にする
static JsonObject Lists(object? parent)
{
    var lists = new JsonObject();
    if (parent is null)
    {
        return lists;
    }
    foreach (var property in parent.GetType().GetProperties())
    {
        if (!property.PropertyType.IsArray || property.GetValue(parent) is not Array items)
        {
            continue;
        }
        // 部材の一覧（StbColumns など）は、子の要素の名前（XmlArrayItem）で書く
        var item = property.GetCustomAttribute<XmlArrayItemAttribute>()?.ElementName;
        lists[string.IsNullOrEmpty(item) ? ElementName(property, property.PropertyType.GetElementType()!) : item] = Elements(items);
    }
    return lists;
}

static JsonArray Elements(IEnumerable? items)
{
    var array = new JsonArray();
    foreach (var item in items ?? Array.Empty<object>())
    {
        array.Add(Attributes(item));
    }
    return array;
}

// 要素の属性と、子の要素の属性（断面の形を書く子は、子の名前の下に入れ子で書く）
static JsonObject Attributes(object item)
{
    var found = new JsonObject();
    var type = item.GetType();
    foreach (var property in type.GetProperties())
    {
        var value = property.GetValue(item);
        if (value is null || property.Name.EndsWith("Specified", StringComparison.Ordinal))
        {
            continue;
        }
        var specified = type.GetProperty(property.Name + "Specified");
        if (specified is not null && specified.GetValue(item) is false)
        {
            continue;
        }
        var attribute = property.GetCustomAttribute<XmlAttributeAttribute>();
        if (attribute is null)
        {
            // 子の要素。xs:choice（Item・Items）は、値の型に合う XmlElement の名前で書く
            if (value is Array children)
            {
                foreach (var child in children)
                {
                    var key = ElementName(property, child.GetType());
                    if (found[key] is not JsonArray list)
                    {
                        list = new JsonArray();
                        found[key] = list;
                    }
                    list.Add(Child(child));
                }
            }
            else if (!value.GetType().IsPrimitive && value is not string && !value.GetType().IsEnum)
            {
                found[ElementName(property, value.GetType())] = Child(value);
            }
            continue;
        }
        var @default = property.GetCustomAttribute<DefaultValueAttribute>();
        if (@default is not null && Equals(@default.Value, value))
        {
            continue;
        }
        var name = string.IsNullOrEmpty(attribute.AttributeName) ? property.Name : attribute.AttributeName;
        found[name] = Scalar(value);
    }
    return found;
}

static string ElementName(PropertyInfo property, Type type)
{
    var elements = property.GetCustomAttributes<XmlElementAttribute>().ToList();
    var match = elements.FirstOrDefault(e => e.Type == type) ?? (elements.Count == 1 ? elements[0] : null);
    return string.IsNullOrEmpty(match?.ElementName) ? property.Name : match.ElementName;
}

static JsonNode Child(object value)
{
    var child = Attributes(value);
    child["$type"] = value.GetType().Name;
    return child;
}

static JsonNode? Scalar(object value) => value switch
{
    double d => JsonValue.Create(d),
    bool b => JsonValue.Create(b),
    int i => JsonValue.Create(i),
    Enum e => JsonValue.Create(EnumName(e)),
    _ => JsonValue.Create(Convert.ToString(value, CultureInfo.InvariantCulture)),
};

static string EnumName(Enum value)
{
    var name = value.ToString();
    var field = value.GetType().GetField(name);
    var xml = field?.GetCustomAttribute<XmlEnumAttribute>()?.Name;
    return string.IsNullOrEmpty(xml) ? name : xml;
}

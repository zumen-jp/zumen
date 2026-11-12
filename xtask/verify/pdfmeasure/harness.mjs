// 外部の定義本体は setup.mjs が target へ抽出する。このファイルは入出力と型の代替だけを持つ。
export const harness = String.raw`
#include <cmath>
#include <cstdlib>
#include <iomanip>
#include <iostream>
#include <limits>
#include <locale>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

struct Sample { char group; std::size_t index; double value; bool last; };
struct QString { std::vector<Sample> samples; };
struct QChar { explicit QChar(char) {} };
struct QStringList {
    std::vector<Sample> samples;
    QStringList& operator<<(const QString& part) {
        samples.insert(samples.end(), part.samples.begin(), part.samples.end());
        return *this;
    }
    QString join(QChar) const { return {samples}; }
};
struct QPointF {
    double first, second;
    double x() const { return first; }
    double y() const { return second; }
};

// Qt 6.11 の公開 API 文書による double の境界。Qt のソースは使用しない。
// https://doc.qt.io/qt-6.11/qtnumeric.html#qFuzzyIsNull
bool qFuzzyIsNull(double value) { return std::abs(value) <= 1e-12; }

namespace pdf {
using PDFReal = double;
class PDFNumberFormat {
public:
    PDFReal factor = 1.0;
    char group = 'D';
    std::size_t index = 0;
    PDFReal getConversionFactor() const { return factor; }
    QString formatValue(PDFReal value, bool last) const { return {{{group, index, value, last}}}; }
    static QString format(const std::vector<PDFNumberFormat>& formats, PDFReal value);
};
class PDFMeasure {
public:
    std::vector<PDFNumberFormat> m_x, m_y, m_distance;
    PDFReal m_factorYX = 1.0;
    PDFReal getUnitsPerUserSpaceUnit() const;
    PDFReal getUnitsPerUserSpaceUnitY() const;
    QString formatDistance(PDFReal userSpaceLength) const;
    QString formatDistance(const QPointF& userSpaceVector) const;
};
}

#include "upstream-functions.inc"

std::string token() {
    std::string text;
    if (!(std::cin >> text)) throw std::runtime_error("入力が途中で終わりました");
    return text;
}
double number() {
    const std::string text = token();
    char* end = nullptr;
    const double value = std::strtod(text.c_str(), &end);
    if (end != text.c_str() + text.size() || !std::isfinite(value))
        throw std::runtime_error("有限の数が必要です");
    return value;
}
std::vector<pdf::PDFNumberFormat> formats(char group) {
    const double raw = number();
    if (raw < 0.0 || raw > 256.0 || std::trunc(raw) != raw)
        throw std::runtime_error("書式の数が上限を超えました");
    const auto count = static_cast<std::size_t>(raw);
    std::vector<pdf::PDFNumberFormat> result;
    result.reserve(count);
    for (std::size_t i = 0; i < count; ++i) result.push_back({number(), group, i});
    return result;
}
int main(int argc, char** argv) {
    if (argc == 2 && std::string(argv[1]) == "--check") return 0;
    std::cin.imbue(std::locale::classic());
    std::cout.imbue(std::locale::classic());
    std::cout << std::setprecision(std::numeric_limits<double>::max_digits10);
    try {
        std::string mode;
        std::size_t cases = 0;
        while (std::cin >> mode) {
            if (++cases > 1024 || (mode != "scalar" && mode != "vector"))
                throw std::runtime_error("計測の種類または件数が不正です");
            const double first = number(), second = number();
            pdf::PDFMeasure measure;
            measure.m_factorYX = number();
            measure.m_x = formats('X');
            measure.m_y = formats('Y');
            measure.m_distance = formats('D');
            const QString result = mode == "scalar" ? measure.formatDistance(first)
                : measure.formatDistance(QPointF{first, second});
            for (const Sample& sample : result.samples)
                if (!std::isfinite(sample.value)) throw std::runtime_error("計算結果が有限ではありません");
            std::cout << "{\"mode\":\"" << mode << "\",\"samples\":[";
            bool comma = false;
            for (const Sample& sample : result.samples) {
                if (comma) std::cout << ',';
                comma = true;
                std::cout << "{\"array\":\"" << sample.group << "\",\"index\":" << sample.index
                    << ",\"value\":" << sample.value << ",\"last\":" << (sample.last ? "true" : "false") << '}';
            }
            std::cout << "]}\n";
        }
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
`;

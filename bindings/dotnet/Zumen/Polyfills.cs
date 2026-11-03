// netstandard2.0 にない型のうち、コンパイラーが使うものをラッパーの中に置く

#if !NET
namespace System.Runtime.CompilerServices
{
    /// <summary>record の init のために、コンパイラーが引く</summary>
    internal static class IsExternalInit
    {
    }
}
#endif

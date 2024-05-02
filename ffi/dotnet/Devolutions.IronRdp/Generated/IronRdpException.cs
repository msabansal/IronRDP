// <auto-generated/> by Diplomat

#pragma warning disable 0105
using System;
using System.Runtime.InteropServices;

using Devolutions.IronRdp.Diplomat;
#pragma warning restore 0105

namespace Devolutions.IronRdp;

#nullable enable

public partial class IronRdpException : Exception
{
    private IronRdpError _inner;

    public IronRdpException(IronRdpError inner) : base(inner.ToDisplay())
    {
        _inner = inner;
    }

    public IronRdpError Inner
    {
        get
        {
            return _inner;
        }
    }
}
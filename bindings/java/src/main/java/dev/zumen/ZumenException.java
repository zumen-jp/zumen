package dev.zumen;

/** 境界の関数が ok でない状態を返した */
public final class ZumenException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    private final String operation;
    private final int code;
    private final byte[] data;

    ZumenException(String operation, int code, byte[] data) {
        super(operation + " が " + Constants.Status.name(code) + " を返した");
        this.operation = operation;
        this.code = code;
        this.data = data;
    }

    /** 呼んだ境界の関数 */
    public String getOperation() {
        return operation;
    }

    /** 状態コード（{@link Constants.Status}） */
    public int getCode() {
        return code;
    }

    /** 状態コードの名前 */
    public String getStatus() {
        return Constants.Status.name(code);
    }

    /** 応答の JSON のバイト列。書かれなかったときは null */
    public byte[] getData() {
        return data == null ? null : data.clone();
    }

    /** 応答。書かれなかったときは null */
    public Messages.Response getResponse() {
        return data == null ? null : Messages.Response.of(Json.parse(data));
    }
}

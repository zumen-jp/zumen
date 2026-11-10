// 生成済みの blob だけの往復。既存の scene job の生成費用とは分ける。
self.onmessage = ({ data: { mode, buffer } }) => self.postMessage(buffer, mode === "transfer" ? [buffer] : []);
self.postMessage("ready");

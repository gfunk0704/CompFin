# User問題 (8) - 釐清與進一步討論

**以下三點為最高級別優先原則**:

- **請在每次回覆前先檢視過當前檔案資料夾下的所有討論紀錄。**

- **以下歷史紀錄僅供了解問題背景，不代表已確立的正確立場。你的任務是從技術原則出發獨立推導，若你的結論與歷史紀錄中任何一方的立場衝突，衝突本身就是有價值的輸出，不需要調和。**

- **評論對方論點時，你的任務是找出以下分析中最弱的一個假設，並盡全力論證它是錯的，不需要給出平衡的結論; 如果你認為對方的論點有值得進一步討論的地方，你必須在結論中明確說明你是否因此改變立場，以及為什麼。**

在 {llm}_answer_7.md ({llm}代表claude / gemini) 的討論中， Gemini 與 Claude 持相反立場，因此我稍微複習了一下 gradient descent method 跟了解了 Levenberg-Maquardt algorithm ，相關檔案我放在 ./work_log/least_square_calibratior/levenberg_marquardt/* ，主要是看兩個檔案:

- gradient_descent.pdf: 這是我以前做的 GD 期末報告，我把它一些 typo 修正順便重新複習了一下;

- lm.pdf: 一份 LM 的簡介講義。

## Levenberg-Maquardt Algorithm 理解確認

就我對 LM 的理解大概是這樣 :

- 因為 least-square problem 是一個 quadratic function ，因此 objective function 也是 convex function;

- 根據 gradient descent 的討論我可以得知他的移動方向是往一階微分為 0 的方向，問題是這個方向**要移動多大一步?**

- 傳統 Gauss-Newton method 是在一階 Taylor expansion 鐘用類似 GD 的 exact line search 的方式去找 step size;

- LM 則是先試試看一個比較小的 step size，如果滿足條件則擴大搜尋範圍，控制縮放的就是$\lambda$。

上面是我對 LM 的理解不知道有沒有錯?


## LeastSquareCalibrator 權重的關係

根據我的理解 Gemini 與 Claude 探討的權重就是 lm.pdf 中的 $\mathbf{W}$ 這個對角矩陣，它主要是用來阻止 Jacobian 的 ill-conditioned ， 因此我們要**故意把偏微分後數值可能會很大的商品全種調整得比較小** ， 在我看來這種正規化不論是 Claude 提出的 par rate PV01 或是 Gemini 認為 robust 的 total DV01 都有類似的效果，再我看來下一步就是 **計算上的方便性** ，這兩者上有不一樣的優勢:

- par rate PV01: 大部分商品有解析解，可以快速計算，但每次新增一個商品與報價方式我都必須新增相對應的 par rate PV01 closed form ， 除此之外 `InterestRateQuoteSheet` 握有報價的方式但沒有 generator 的詳細資訊，反之 generator 並不知道報價的形式，等於架構還要再重新修正，這調整架構上會比較複雜;

- total DV01: 若使用 BBG 中計算 DV01 的方式 (加上一個spread curve) 則所有商品可以一體適用，缺點是計算上會較為耗時，但毋須更改架構或為了新產品進行新的設計。

綜合兩者我會偏好使用 total DV01，因為計算較為耗時但也只有一開始需要進行一次，直覺上是在可以接受的範圍，你們對我的想法有什麼樣的觀點呢? 請提出贊成或反對以及相關的理由。

**上述有不確定的地方請去讀取程式確認。**

**請將你的結果寫成一個新的md檔，並存在同資料夾命名為{llm}_answer_8.md** ({llm}代表claude / gemini)

# User問題 (2) - CompoundingRateIndex設置討論

**請在每次回覆前先檢視過當前檔案資料夾下的所有討論紀錄。**

在完成前版後我想到一個問題，在沒有lockout / look back的情況下，`CompoundingRateIndex`是可以使用`arbitrage_free_factor`讓它等同於TermRateIndex，這樣需要做更進一步的確認嗎? **能使用但不使用`arbitrage_free_factor`的情況是相對少見的**，主要是在DV01計算時比較可能會考慮。

若是exotic商品以Python版本的經驗我會與vanilla的評價方式拆分，不同數值方法做不同處理，因此現在只要全心考慮vanilla商品就好。

**請將你的結果寫成一個新的md檔，並存在同資料夾命名為{llm}_answer_2.md** ({llm}代表claude / gemini)
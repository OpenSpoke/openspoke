package WordingChecker {
    doc /* 2026-10-30 Hasegawa 指摘「検査器って変な日本語です」。
           Claude が checker / validator を「検査器」と漢字に訳した。
           既存 topic claude_japanese_wording_no_invented_kanji と同じ型の誤り。 */

    part def WordChoice {
        attribute avoid : ScalarValues::String;
        attribute use : ScalarValues::String;
    }
    part checker : WordChoice { :>> avoid = "検査器"; :>> use = "チェッカー (構文だけを見るものは構文チェッカー)"; }
    part validator : WordChoice { :>> avoid = "検査器"; :>> use = "バリデーター"; }
    part negativeControl : WordChoice { :>> avoid = "否定的対照 (利用者向けの説明で)"; :>> use = "わざと壊した入力"; }
}

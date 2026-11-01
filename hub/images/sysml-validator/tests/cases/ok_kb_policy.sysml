package KnowledgeBaseNotationPolicy {
    doc /* 知識ベースの書き方の方針。2026-10-30 Hasegawa 決定。
           経緯の topic: knowledge_base_sysml_v2_proposal_20261030、
           knowledge_base_sysml_v2_token_measurement_20261030、
           knowledge_base_sysml_v2_reveals_inconsistency_20261030。 */

    enum def Notation { markdown; sysmlV2Textual; }

    part def Topic {
        attribute recordedOnOrAfter20261030 : ScalarValues::Boolean;
        attribute notation : Notation;
    }

    part def Checker {
        attribute name : ScalarValues::String;
        attribute version : ScalarValues::String;
        attribute covers : ScalarValues::String;
    }
    part syntaxChecker : Checker {
        :>> name = "sysml2 (crates.io、命令 sysml check)"; :>> version = "0.3.1";
        :>> covers = "構文のみ。型や参照の解決は見ない";
    }
    part semanticChecker : Checker {
        :>> name = "mercurio-sysml (crates.io、SysmlEnvironment::compile_text)"; :>> version = "0.86.0";
        :>> covers = "構文と、型・再定義先・式中の名前の解決";
    }

    requirement def NotationRule;

    requirement newTopicsInSysML : NotationRule {
        doc /* 2026-10-30 以降に記録する知見は、本文を SysML v2 のテキスト記法で書く。
               器は従来どおり Memgraph の Topic ノードの content。 */
        subject t : Topic;
        require constraint { t.recordedOnOrAfter20261030 implies t.notation == Notation::sysmlV2Textual }
    }
    requirement keepExistingTopics : NotationRule {
        doc /* 2026-10-30 より前の topic は Markdown のまま残し、書き換えない。 */
    }
    requirement checkBeforeRecord : NotationRule {
        doc /* 記録の前に syntaxChecker と semanticChecker の両方を通す。
               片方だけでは足りない (sysml2 は未解決の参照を通し、mercurio は名前の無い属性を通す)。 */
    }
    requirement narrativeInDoc : NotationRule {
        doc /* 経緯・手順・コマンド全文・実測ログなど構造にできない内容は、
               該当要素の doc コメントに詳細のまま入れる (claude_kb_recording_principles の
               「要約でなく詳細」は維持)。 */
    }
    requirement generateFromSource : NotationRule {
        doc /* 実測データ (kubectl の出力等) から起こすモデルは手で書かず、
               取得データから機械的に生成するスクリプトで出力する。 */
    }
}

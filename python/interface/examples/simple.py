import torch

from ltp import LTP


def legacy():
    ltp = LTP("LTP/legacy")
    ltp.add_word("汤姆去")
    result = ltp(
        ["他叫汤姆去拿外衣。", "树上停着一些小鸟。先飞走了19只，又飞走了15只。两次共飞走了多少只小鸟？"],
        tasks=["cws", "pos", "ner"],
    )
    print(result.cws)
    print(result.pos)
    print(result.ner)


def neural():
    ltp = LTP("LTP/tiny")

    if torch.cuda.is_available():
        ltp = ltp.to("cuda")

    ltp.add_word("汤姆去")

    # 未分词的文本
    result = ltp.pipeline(
        ["他叫汤姆去拿外衣。", "韓語：한국의 단오", "树上停着一些小鸟。先飞走了19只，又飞走了15只。两次共飞走了多少只小鸟？"],
        tasks=["cws", "pos", "ner", "srl", "dep", "sdp"],
    )
    print(result.cws)
    print(result.pos)
    print(result.ner)
    print(result.srl)
    print(result.dep)
    print(result.sdp)

    # 已经分词的文本
    result = ltp.pipeline(
        [["他", "叫", "汤姆", "去", "拿", "外衣", "。"], ["가을동", "叫", "1993", "年", "的", "Ameri", "·"]],
        # 注意这里移除了 "cws" 任务
        tasks=["pos", "ner", "srl", "dep", "sdp"],
    )
    print(result.pos)
    print(result.ner)
    print(result.srl)
    print(result.dep)
    print(result.sdp)


def issue590():
    ltp = LTP("LTP/tiny")
    ltp.add_words(words=["[ENT]"])
    print(ltp.pipeline(["[ENT] Info"], tasks=["cws"]))

    ltp.add_words(words=["[EOS]"])
    print(ltp.pipeline(["[EOS] Info"], tasks=["cws"]))


def issue592():
    legacy_ltp = LTP("LTP/legacy")

    legacy_ltp.add_words(words=['SCSG', 'IP地址'])
    print(legacy_ltp.pipeline(['SCSGIP地址'], tasks=["cws"]))

    neural_ltp = LTP("LTP/tiny")

    # not bug, but not work because of the bert tokenizer
    neural_ltp.add_words(words=['SCSG', 'IP地址'])
    print(neural_ltp.pipeline(['SCSGIP地址'], tasks=["cws"]))


def main():
    # legacy()
    # neural()
    # issue590()
    issue592()


if __name__ == "__main__":
    main()

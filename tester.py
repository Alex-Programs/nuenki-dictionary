import requests


def test(word, lang):
    url = "http://localhost:9042/get_definition"

    r = requests.get(url + f"?language={lang}&word={word}")

    print(r.status_code)
    print(r.content)


test("Schlecht", "German")

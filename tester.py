import requests
import time


def test(word, lang):
    url = "https://dictionary.nuenki.app/get_definition"  # "http://localhost:9042/get_definition"

    r = requests.get(url + f"?language={lang}&word={word}")

    print(r.status_code)
    print(r.content)


for i in range(1, 100):
    time.sleep(10)
    test("Schlecht" + str(i), "German")

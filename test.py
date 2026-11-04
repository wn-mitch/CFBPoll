import sys
import requests
from bs4 import BeautifulSoup


def print_grid(url):
    response = requests.get(url, timeout=30)
    response.raise_for_status()
    soup = BeautifulSoup(response.content, "html.parser")

    points = {}
    for row in soup.find("table").find_all("tr")[1:]:
        x, char, y = [cell.get_text() for cell in row.find_all("td")]
        points[int(x), int(y)] = char

    width = max(x for x, y in points) + 1
    for y in range(max(y for x, y in points), -1, -1):
        print("".join(points.get((x, y), " ") for x in range(width)))


if __name__ == "__main__":
    print_grid(sys.argv[1])

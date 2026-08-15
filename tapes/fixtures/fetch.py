# The same call site in python. The tail is load-bearing: `7c-python.tape`
# counts back to the call (`G k O`) — one `k` fewer than its two siblings,
# because this body ends with the call rather than with a brace after it.

from asyncio import gather

from .json import fetch_json
from .retry import RetryPolicy


async def fetch_all(urls: list[str]) -> list[dict]:
    return await gather(*(fetch_json(url) for url in urls))

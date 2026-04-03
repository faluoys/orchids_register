import argparse

import uvicorn


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run mail-gateway FastAPI server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8081)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    uvicorn.run("mail_gateway.app:app", host=args.host, port=args.port, reload=False)


if __name__ == "__main__":
    main()

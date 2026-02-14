FROM ubuntu:plucky-20250415
WORKDIR /src
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
RUN pip install --no-cache-dir -r requirements-mainserver.txt
COPY /src/main.rs /app/
CMD ["python", "./app/mainserver.py"] 
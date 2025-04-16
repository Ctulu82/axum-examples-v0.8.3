-- up.sql: DB 마이그레이션 시 테이블 생성
CREATE TABLE "users" (
    "id" SERIAL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "hair_color" TEXT
);

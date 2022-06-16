const puppeteer = require("puppeteer");

let browser;

async function initBrowser() {
  if (!browser) {
    browser = await puppeteer.launch();
  }
  const context = await browser.createIncognitoBrowserContext();
  const page = await context.newPage();
  await page.goto("http://localhost:3000");
  return [context, page];
}

test("init", async () => {
  const [ctx, page] = await initBrowser();

  const version = await page.evaluate(async () => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const cc = await CoreCrypto.init({
      databaseName: "test init",
      key: "test",
      clientId: "test",
    });

    return CoreCrypto.version();
  });

  expect(version).toBe("0.3.0");

  await page.close();
  await ctx.close();
});

test("get client public key", async () => {
  const [ctx, page] = await initBrowser();

  const pkLength = await page.evaluate(async () => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const cc = await CoreCrypto.init({
      databaseName: "get client public key",
      key: "test",
      clientId: "test",
    });

    const len = cc.clientPublicKey().length;
    await cc.wipe();
    return len;
  });

  expect(pkLength).toBe(32);

  await page.close();
  await ctx.close();
});

test("get client keypackages", async () => {
  const [ctx, page] = await initBrowser();

  const kpNumber = await page.evaluate(async () => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const cc = await CoreCrypto.init({
      databaseName: "get client keypackages",
      key: "test",
      clientId: "test",
    });

    const kps = await cc.clientKeypackages(20);
    const len = kps.length;

    await cc.wipe();

    return len;
  });

  expect(kpNumber).toBe(20);

  await page.close();
  await ctx.close();
});

test("encrypt message", async () => {
  const [ctx, page] = await initBrowser();

  const msgLen = await page.evaluate(async () => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const cc = await CoreCrypto.init({
      databaseName: "encrypt message",
      key: "test",
      clientId: "test",
    });

    const encoder = new TextEncoder();

    const conversationId = encoder.encode("testConversation");

    await cc.createConversation(conversationId);

    const encryptedMessage = await cc.encryptMessage(
      conversationId,
      encoder.encode("Hello World!")
    );

    const len = encryptedMessage.length;

    await cc.wipe();

    return len;
  });

  expect(msgLen).toBeGreaterThan(0);

  await page.close();
  await ctx.close();
});

test("roundtrip message", async () => {
  const [ctx, page] = await initBrowser();
  const [ctx2, page2] = await initBrowser();

  const messageText = "Hello World!";

  let kp = await page2.evaluate(async () => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const cc2 = await CoreCrypto.init({
      databaseName: "roundtrip message test 2",
      key: "test2",
      clientId: "test2",
    });

    const [kp] = await cc2.clientKeypackages(1);
    await cc2.close();
    return kp;
  });

  kp = Uint8Array.from(Object.values(kp));

  let [welcome, message] = await page.evaluate(async (kp) => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const cc = await CoreCrypto.init({
      databaseName: "roundtrip message test 1",
      key: "test",
      clientId: "test",
    });

    const encoder = new TextEncoder();

    const conversationId = "testConversation";
    const conversationIdBuffer = encoder.encode(conversationId);

    await cc.createConversation(conversationIdBuffer);

    const welcome = await cc.addClientsToConversation(conversationIdBuffer, [
      { id: encoder.encode("test2"), kp: Uint8Array.from(Object.values(kp)) },
    ]);

    if (!welcome) {
      throw new Error("no welcome message was generated");
    }

    const messageText = "Hello World!";

    const message = await cc.encryptMessage(
      conversationIdBuffer,
      encoder.encode(messageText)
    );

    return [welcome, message];
  }, kp);

  welcome.welcome = Uint8Array.from(welcome.welcome);
  welcome.message = Uint8Array.from(welcome.message);
  message = Uint8Array.from(Object.values(message));

  const isMessageIdentical = await page2.evaluate(async (welcome, message) => {
    const { CoreCrypto } = await import("./corecrypto.js");

    const encoder = new TextEncoder();

    const cc2 = await CoreCrypto.init({
      databaseName: "roundtrip message test 2",
      key: "test2",
      clientId: "test2",
    });

    const messageBuffer = encoder.encode("Hello World!");

    const conversationId = "testConversation";
    const conversationIdBuffer = encoder.encode(conversationId);

    const welcomeConversationId = await cc2.processWelcomeMessage(Uint8Array.from(Object.values(welcome)));
    if (!conversationIdBuffer.every((val, i) => val === welcomeConversationId[i])) {
      throw new Error(`conversationId mismatch, got ${welcomeConversationId}, expected ${conversationIdBuffer}`);
    }

    const decryptedMessage = await cc2.decryptMessage(welcomeConversationId, Uint8Array.from(Object.values(message)));

    return decryptedMessage.every((val, i) => val === messageBuffer[i]);
  }, welcome.welcome, message);

  expect(isMessageIdentical).toBe(true);

  await page.close();
  await page2.close();
  await ctx.close();
  await ctx2.close();
}, 20000);

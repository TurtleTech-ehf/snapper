{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE OverloadedStrings #-}

-- | C-callable surface: parse markup with the pandoc Haskell library and
-- return the pandoc JSON AST as a UTF-8 C string.
module SnapperPandoc
  ( snapper_pandoc_parse
  , snapper_pandoc_free
  , snapper_pandoc_hs_ready
  ) where

import Control.Exception (SomeException, try)
import qualified Data.Aeson as Aeson
import qualified Data.ByteString as BS
import qualified Data.ByteString.Lazy as BL
import qualified Data.ByteString.Unsafe as BU
import Data.Text (Text)
import qualified Data.Text as T
import qualified Data.Text.Encoding as TE
import Foreign.C.String (CString)
import Foreign.C.Types (CChar)
import Foreign.Marshal.Alloc (free, mallocBytes)
import Foreign.Marshal.Utils (copyBytes)
import Foreign.Ptr (Ptr, nullPtr)
import Foreign.Storable (poke, pokeByteOff)
import Text.Pandoc
  ( PandocError
  , ReaderOptions
  , def
  , getDefaultExtensions
  , readHtml
  , readLaTeX
  , readMarkdown
  , readOrg
  , readRST
  , readTypst
  , runPure
  )
import Text.Pandoc.Class (PandocPure)
import Text.Pandoc.Definition (Pandoc)
import Text.Pandoc.Options (readerExtensions)

-- | Exported for linkers that want an explicit touch of the Haskell side.
snapper_pandoc_hs_ready :: IO ()
snapper_pandoc_hs_ready = pure ()

foreign export ccall snapper_pandoc_hs_ready :: IO ()

-- | Parse @format@ + @input@ to pandoc JSON. See snapper_pandoc.h.
snapper_pandoc_parse :: CString -> CString -> Ptr CString -> IO CString
snapper_pandoc_parse fmtPtr inPtr errOut = do
  fmtBs <- BS.packCString fmtPtr
  inBs <- BS.packCString inPtr
  let fmt = TE.decodeUtf8Lenient fmtBs
      input = TE.decodeUtf8Lenient inBs
  result <- try (pure $! runParse fmt input) :: IO (Either SomeException (Either String BL.ByteString))
  case result of
    Left ex -> failWith errOut (show ex)
    Right (Left err) -> failWith errOut err
    Right (Right json) -> do
      clearErr errOut
      blToCString json

foreign export ccall snapper_pandoc_parse :: CString -> CString -> Ptr CString -> IO CString

snapper_pandoc_free :: CString -> IO ()
snapper_pandoc_free ptr =
  if ptr == nullPtr then pure () else free ptr

foreign export ccall snapper_pandoc_free :: CString -> IO ()

runParse :: Text -> Text -> Either String BL.ByteString
runParse fmt input =
  case lookupReader fmt of
    Nothing -> Left $ "unsupported pandoc input format: " <> T.unpack fmt
    Just reader ->
      case runPure (reader opts input) of
        Left err -> Left (show (err :: PandocError))
        -- Compact JSON; force encode before returning (hot path for FFI).
        Right doc -> Right $! Aeson.encode (doc :: Pandoc)
  where
    opts :: ReaderOptions
    opts = def { readerExtensions = getDefaultExtensions fmt }

-- | Map format names used by snapper / pandoc CLI to readers.
lookupReader :: Text -> Maybe (ReaderOptions -> Text -> PandocPure Pandoc)
lookupReader fmt =
  case T.toLower fmt of
    "markdown" -> Just readMarkdown
    "gfm" -> Just readMarkdown
    "commonmark" -> Just readMarkdown
    "org" -> Just readOrg
    "rst" -> Just readRST
    "latex" -> Just readLaTeX
    "html" -> Just readHtml
    "typst" -> Just readTypst
    _ -> Nothing

failWith :: Ptr CString -> String -> IO CString
failWith errOut msg = do
  cmsg <- stringToCString msg
  if errOut /= nullPtr
    then poke errOut cmsg
    else free cmsg
  pure nullPtr

clearErr :: Ptr CString -> IO ()
clearErr errOut =
  if errOut == nullPtr then pure () else poke errOut nullPtr

stringToCString :: String -> IO CString
stringToCString s = blToCString (BL.fromStrict (TE.encodeUtf8 (T.pack s)))

blToCString :: BL.ByteString -> IO CString
blToCString bl = do
  let bs = BL.toStrict bl
      len = BS.length bs
  ptr <- mallocBytes (len + 1)
  BU.unsafeUseAsCStringLen bs $ \(src, _) -> do
    copyBytes ptr src len
    pokeByteOff ptr len (0 :: CChar)
  pure ptr

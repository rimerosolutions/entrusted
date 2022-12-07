Feature: Default test data

  Scenario: When we convert default test files
    Given a set of files to convert
      | filename         |
      | sample-doc.doc   |
      | sample-docx.docx |
      | sample-gif.gif   |
      | sample-jpeg.jpeg |
      | sample-jpg.jpg   |
      | sample-odg.odg   |
      | sample-odp.odp   |
      | sample-ods.ods   |
      | sample-odt.odt   |
      | sample-pdf.pdf   |
      | sample-png.png   |
      | sample-ppt.ppt   |
      | sample-pptx.pptx |
      | sample-tiff.tiff |
      | sample-xls.xls   |
      | sample-xlsx.xlsx |

    When files are converted
    Then the conversion is successful
